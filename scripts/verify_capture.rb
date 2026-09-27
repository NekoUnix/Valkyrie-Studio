require_relative '../lib/live2d_studio'
require_relative '../lib/live2d_studio/native'
include Live2D
raise 'GLFW init failed' if GLFW.glfwInit.zero?
GLFW.glfwWindowHint(0x00022002, 3)
GLFW.glfwWindowHint(0x00022003, 3)
GLFW.glfwWindowHint(0x00022008, 0x00032001)
GLFW.glfwWindowHint(0x00022006, 1)
GLFW.glfwWindowHint(0x00020004, 0)
window = GLFW.glfwCreateWindow(160, 160, 'Capture verification', nil, nil)
begin
  raise 'Window failed' if window.null?
  GLFW.glfwMakeContextCurrent(window)
  Native.check(Native.l2d_init)
  renderer = Renderer.new(96, 160)
  renderer.clear([0.1, 0.4, 0.7, 0.3])
  Native.check(Native.l2d_diagnostic(renderer.fbo, 0.7))
  reference = renderer.rgba.bytes
  raise 'Submit failed' unless renderer.capture_submit(37)
  result = nil
  Timeout.timeout(5) { sleep 0.001 until (result = renderer.capture_poll) }
  raise 'Timestamp changed' unless result[1] == 37
  values = result[0].bytes
  difference = reference.zip(values).map { |a, b| (a - b).abs }.max
  raise "Orientation/color/alpha mismatch: #{difference}" if difference > 1
  raise 'Empty ring should not block' unless renderer.capture_poll.nil?
  3.times { |i| raise 'Ring slot failed' unless renderer.capture_submit(i) }
  raise 'Full GPU ring should decline new frame' if renderer.capture_submit(4)
  Native.l2d_capture_reset
  raise 'Reset left stale frames' unless renderer.capture_poll.nil?
  puts "GPU capture: orientation, straight alpha, color, timestamp, bounded ring and reset passed (max byte difference #{difference})."
ensure
  renderer&.close
  Native.l2d_shutdown
  GLFW.glfwDestroyWindow(window) unless window.null?
  GLFW.glfwTerminate
end
