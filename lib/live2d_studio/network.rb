# frozen_string_literal: true
module Live2D
  # Network workers never call FFI. Latest tracking frames are coalesced; control
  # commands enter a bounded queue consumed exclusively by the render thread.
  class Inbox
    def initialize
      @mutex, @tracking, @commands = Mutex.new, {}, SizedQueue.new(128)
    end
    def tracking(source, values)
      @mutex.synchronize { @tracking[source] = values }
    end
    def push(command, reply = nil)
      @commands.push([command, reply], true)
    rescue ThreadError
      raise Error, 'Command queue full; retry later'
    end
    def drain
      frames = @mutex.synchronize { current = @tracking; @tracking = {}; current }
      commands = []
      128.times do
        break if @commands.empty?
        commands << @commands.pop(true)
      end
      [frames, commands]
    end
  end

  class Network
    attr_reader :errors, :token
    def initialize(config, inbox, token: ENV['L2D_API_TOKEN'])
      @config, @inbox = config, inbox
      @token = token.to_s.empty? ? SecureRandom.hex(24) : token
      @errors, @sockets, @threads = Queue.new, [], []
      @running = false
      @udp_sockets, @udp_threads, @received = [], [], {}
      @tracking_mutex = Mutex.new
      @disabled_sources = []
    end

    def start
      @running = true
      start_tracking(@config.fetch('tracking'))
      api = @config.fetch('api')
      server = TCPServer.new(api.fetch('bind'), api.fetch('port'))
      @sockets << server
      @threads << Thread.new { serve(server, api) }
      self
    rescue StandardError
      stop
      raise
    end

    def start_tracking(track)
      track.fetch('udp_ports').each do |port|
        socket = UDPSocket.new
        socket.bind(track.fetch('bind'), port)
        @udp_sockets << socket
        @udp_threads << Thread.new do
          while @running
            next unless IO.select([socket], nil, nil, 0.2)
            begin
            packet, peer = socket.recvfrom(65_537)
            # Webcam helper always identifies its own source, only over loopback.
            source = 'phone'
            if packet.start_with?('{')
              obj = JSON.parse(packet)
              source = 'webcam' if obj['source'] == 'webcam' && ['127.0.0.1', '::1'].include?(peer[3])
            end
            values = Tracking::Parser.parse(packet)
            next if values.empty?
            next if @tracking_mutex.synchronize { @disabled_sources.include?(source) }
            @tracking_mutex.synchronize do
              last = @received[source] || { 'packets' => 0 }
              @received[source] = { 'packets' => last['packets'] + 1, 'at' => Live2D.monotonic, 'peer' => peer[3] }
            end
            @inbox.tracking(source, values)
          rescue Error, JSON::ParserError
            next # Bad UDP packets are untrusted and never terminate the listener.
            end
          end
        rescue IOError, Errno::EBADF, Errno::ENOTSOCK
          nil
        end
      end
    end

    def tracking_status
      @tracking_mutex.synchronize do
        @received.transform_values { |v| v.reject { |k, _| k == 'at' }.merge('age' => Live2D.monotonic - v['at']) }
      end
    end

    def enable_source(source, enabled)
      @tracking_mutex.synchronize do
        @disabled_sources.delete(source)
        @disabled_sources << source unless enabled
        @received.delete(source)
      end
    end

    def configure_tracking(bind, ports)
      previous = @config.fetch('tracking').dup
      stop_tracking
      begin
        start_tracking('bind' => bind, 'udp_ports' => ports)
      rescue StandardError
        stop_tracking
        start_tracking(previous)
        raise
      end
      @config['tracking'] = previous.merge('bind' => bind, 'udp_ports' => ports)
    end

    def stop_tracking
      @udp_sockets.each { |s| s.close rescue nil }
      @udp_threads.each { |t| t.join(1) }
      @udp_sockets.clear
      @udp_threads.clear
    end

    def serve(server, config)
      clients = {}
      while @running
        ready = IO.select([server, *clients.keys], nil, nil, 0.05)
        Array(ready&.first).each do |io|
          if io == server
            client = server.accept_nonblock(exception: false)
            next if client == :wait_readable
            if clients.size >= config.fetch('max_clients')
              client.close
            else
              clients[client] = { input: +'', output: +'', replies: [], seen: Live2D.monotonic }
            end
            next
          end
          state = clients[io]
          chunk = io.read_nonblock(4096, exception: false)
          if chunk.nil?
            close_client(clients, io)
            next
          end
          next if chunk == :wait_readable
          state[:seen] = Live2D.monotonic
          state[:input] << chunk
          if state[:input].bytesize > config.fetch('max_packet_bytes')
            close_client(clients, io)
            next
          end
          loop do
            line_end = state[:input].index("\n")
            break unless line_end
            line = state[:input].slice!(0..line_end)
            begin
              request = JSON.parse(line)
              raise Error, 'Request must be a JSON object' unless request.is_a?(Hash)
              raise Error, 'Invalid API token' unless secure_equal(request['token'].to_s, @token)
              raise Error, 'Too many pending requests' if state[:replies].size >= 16
              reply = Queue.new
              @inbox.push(request, reply)
              state[:replies] << [request['id'], reply]
            rescue Error, JSON::ParserError => e
              state[:output] << JSON.generate(ok: false, error: e.message) << "\n"
            end
          end
        rescue IOError, SystemCallError
          close_client(clients, io)
        end
        clients.keys.each do |io|
          state = clients[io]
          state[:replies].delete_if do |id, reply|
            next false if reply.empty?
            state[:output] << JSON.generate(reply.pop.merge(id: id)) << "\n"
            true
          end
          if state[:output].bytesize > 262_144 || Live2D.monotonic - state[:seen] > 120
            close_client(clients, io)
            next
          end
          unless state[:output].empty?
            written = io.write_nonblock(state[:output], exception: false)
            state[:output].slice!(0, written) if written.is_a?(Integer)
          end
        rescue IOError, SystemCallError
          close_client(clients, io)
        end
      end
    rescue IOError, Errno::EBADF, Errno::ENOTSOCK
      nil
    rescue StandardError => e
      @errors << e.message
    ensure
      clients&.each_key { |socket| socket.close rescue nil }
    end

    def secure_equal(a, b)
      return false unless a.bytesize == b.bytesize
      a.bytes.zip(b.bytes).reduce(0) { |sum, (x, y)| sum | (x ^ y) }.zero?
    end

    def close_client(clients, socket)
      clients.delete(socket)
      socket.close rescue nil
    end

    def stop
      @running = false
      stop_tracking
      @sockets.each { |s| s.close rescue nil }
      @threads.each { |t| t.join(2) }
      @sockets.clear
    end
  end
end
