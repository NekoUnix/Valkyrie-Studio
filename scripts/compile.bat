@echo off
setlocal
cd /d "%~dp0.."
cmake -S native -B build -A x64 %*
if errorlevel 1 exit /b 1
cmake --build build --config Release --target live2d_native_bridge --parallel
if errorlevel 1 exit /b 1
call bundle install
if errorlevel 1 exit /b 1
echo Built. Launch with: bundle exec ruby bin/studio --model "path\model.model3.json"
