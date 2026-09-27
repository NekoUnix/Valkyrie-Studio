# frozen_string_literal: true
require 'socket'
require 'ipaddr'
ip = ARGV.fetch(0) { abort 'Usage: ruby scripts/ifacial_connect.rb IPHONE_IP' }
IPAddr.new(ip)
socket = UDPSocket.new
socket.send('iFacialMocap_sahuasouryya9218sauhuiayeta91555dy3719', 0, ip, 49983)
socket.close
puts 'Requested iFacialMocap streaming. Set tracking.bind to your LAN interface, use phone mode, and allow UDP 49983 in your firewall.'
