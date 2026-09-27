$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$llvm=Join-Path $projectRoot '.tools\llvm-mingw-20260922-ucrt-x86_64\bin'
$cmake=Join-Path $projectRoot '.tools\cmake-4.4.3-windows-x86_64\bin'
if(Test-Path -LiteralPath $llvm){$env:PATH="$llvm;$env:PATH"}
if(Test-Path -LiteralPath $cmake){$env:PATH="$cmake;$env:PATH"}
$compiler=(Get-Command clang.exe).Source
$cppCompiler=(Get-Command clang++.exe).Source
$make=(Get-Command mingw32-make.exe).Source
& cmake -S native -B build -G 'MinGW Makefiles' -DCMAKE_BUILD_TYPE=Release "-DCMAKE_C_COMPILER=$compiler" "-DCMAKE_CXX_COMPILER=$cppCompiler" "-DCMAKE_MAKE_PROGRAM=$make"
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
& cmake --build build --config Release --target live2d_native_bridge --parallel 8
exit $LASTEXITCODE
