# frozen_string_literal: true
require 'ipaddr'
require 'uri'
require 'rbconfig'

module Live2D
  class Connections
    DEFAULTS = {
      'phone_protocol' => 'ifacial', 'phone_ip' => '', 'phone_port' => 49983,
      'listen_bind' => '0.0.0.0', 'listen_port' => 49983, 'vts_url' => 'ws://127.0.0.1:8001',
      'webcam_source' => '0', 'webcam_backend' => 'auto', 'webcam_width' => 640,
      'webcam_height' => 480, 'webcam_fps' => 30, 'webcam_port' => 15483,
      'webcam_preview' => false
    }.freeze
    attr_reader :settings
    def initialize(network, settings)
      @network, @settings, @children = network, DEFAULTS.merge(settings), {}
      @states = { 'phone' => 'Disconnected', 'webcam' => 'Stopped' }
    end

    def configure(values)
      candidate = @settings.merge(values.slice(*DEFAULTS.keys))
      %w[phone_port listen_port webcam_port].each { |k| candidate[k] = Integer(Live2D.number(candidate[k], min: 1, max: 65535)) }
      %w[webcam_width webcam_height].each { |k| candidate[k] = Integer(Live2D.number(candidate[k], min: 160, max: 3840)) }
      candidate['webcam_fps'] = Integer(Live2D.number(candidate['webcam_fps'], min: 1, max: 60))
      raise Error, 'Unknown phone protocol' unless %w[ifacial udp vts].include?(candidate['phone_protocol'])
      raise Error, 'Unknown webcam backend' unless %w[auto dshow msmf v4l2 avfoundation].include?(candidate['webcam_backend'])
      raise Error, 'Enter an IPv4 listen address' unless IPAddr.new(candidate['listen_bind']).ipv4?
      @settings = candidate
    rescue IPAddr::InvalidAddressError
      raise Error, 'Invalid listen IP address'
    end

    def start(source)
      s = @settings
      raise Error, 'Unknown input source' unless %w[phone webcam].include?(source)
      if source == 'phone' && s['phone_protocol'] == 'ifacial'
        raise Error, 'Enter your phone IPv4 address' unless IPAddr.new(s['phone_ip']).ipv4?
      elsif source == 'phone' && s['phone_protocol'] == 'vts'
        uri = URI.parse(s['vts_url'])
        raise Error, 'Enter a ws:// or wss:// VTube Studio API URL' unless %w[ws wss].include?(uri.scheme) && uri.host
      end
      @network.configure_tracking(s['listen_bind'], [s['listen_port'], s['webcam_port']].uniq)
      stop(source)
      @network.enable_source(source, true)
      if source == 'phone'
        case s['phone_protocol']
        when 'ifacial'
          socket = UDPSocket.new
          socket.send('iFacialMocap_sahuasouryya9218sauhuiayeta91555dy3719', 0, s['phone_ip'], s['phone_port'])
          socket.close
          @states[source] = 'Request sent; waiting for tracking'
        when 'vts'
          ruby = RbConfig.ruby
          windowless = ruby.sub(/ruby\.exe\z/i, 'rubyw.exe')
          ruby = windowless if File.file?(windowless)
          spawn_helper(source, ruby, File.join(ROOT, 'bin/vts_bridge'), s['vts_url'], s['listen_port'].to_s)
          @states[source] = 'Bridge started; approve in VTube Studio'
        else
          @states[source] = 'Listening; set the phone destination to this PC'
        end
      else
        python = ENV['L2D_PYTHON'] || File.join(ROOT, '.venv', Gem.win_platform? ? 'Scripts/pythonw.exe' : 'bin/python')
        model = File.join(ROOT, 'vendor/mediapipe/face_landmarker.task')
        raise Error, 'Webcam helper missing. Run scripts/setup_webcam.rb first.' unless File.file?(python) && File.file?(model)
        args = [python, '-u', File.join(ROOT, 'helpers/webcam.py'), '--model', model,
          '--camera', s['webcam_source'], '--backend', s['webcam_backend'], '--width', s['webcam_width'].to_s,
          '--height', s['webcam_height'].to_s, '--fps', s['webcam_fps'].to_s, '--port', s['webcam_port'].to_s]
        args << '--preview' if s['webcam_preview']
        spawn_helper(source, *args)
        @states[source] = 'Starting camera; waiting for a face'
      end
    rescue IPAddr::InvalidAddressError, URI::InvalidURIError
      raise Error, 'Invalid phone IP address or VTube Studio URL'
    ensure
      socket&.close rescue nil
    end

    def spawn_helper(source, *args)
      FileUtils.mkdir_p(File.join(ROOT, 'tmp'))
      log = File.join(ROOT, 'tmp', "#{source}-connection.log")
      pid = Process.spawn(*args, chdir: ROOT, out: log, err: [:child, :out])
      @children[source] = { wait: Process.detach(pid), log: log }
    end

    def stop(source)
      @network.enable_source(source, false)
      if (child = @children.delete(source))
        Process.kill('KILL', child[:wait].pid) if child[:wait].alive?
        child[:wait].join(2)
      end
      @states[source] = 'Stopped'
    rescue Errno::ESRCH
      nil
    end

    def status
      received = @network.tracking_status
      %w[phone webcam].to_h do |source|
        child = @children[source]
        if child && !child[:wait].alive?
          @states[source] = "Helper exited (#{child[:wait].value.exitstatus}); see tmp/#{source}-connection.log"
        end
        packet = received[source] || {}
        live = packet['age'] && packet['age'] < 1
        [source, packet.merge('message' => live ? 'Receiving tracking' : @states[source], 'live' => !!live)]
      end.merge('addresses' => Socket.ip_address_list.select { |a| a.ipv4? && !a.ipv4_loopback? }.map(&:ip_address))
    end

    def close
      %w[phone webcam].each { |source| stop(source) }
    end
  end
end
