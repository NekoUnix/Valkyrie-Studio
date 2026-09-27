source 'https://rubygems.org'
ruby '>= 3.2', '< 4.1'
gem 'ffi', '~> 1.17'
gem 'dotenv', '~> 3.1'
gem 'websocket-client-simple', '~> 0.9'
group :test do
  gem 'minitest', '~> 5.25'
end
# GLFW/OpenGL are bound through FFI; no redundant glfw/opengl gems.
# IO.select and Ruby threads provide bounded native socket concurrency.
