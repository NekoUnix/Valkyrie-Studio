# frozen_string_literal: true
require 'net/http'
require 'digest'
require_relative '../lib/live2d_studio'
url = URI('https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/1/face_landmarker.task')
FileUtils.mkdir_p(File.join(Live2D::ROOT, 'vendor', 'mediapipe'))
output = File.join(Live2D::ROOT, 'vendor', 'mediapipe', 'face_landmarker.task')
Net::HTTP.start(url.host, url.port, use_ssl: true, open_timeout: 10, read_timeout: 90) do |http|
  http.request(Net::HTTP::Get.new(url)) do |response|
    raise Live2D::Error, "Download HTTP #{response.code}" unless response.is_a?(Net::HTTPSuccess)
    File.open(output + '.partial', 'wb') { |file| response.read_body { |chunk| file.write(chunk) } }
  end
end
File.rename(output + '.partial', output)
puts "Saved #{output}\nSHA256: #{Digest::SHA256.file(output).hexdigest}"
