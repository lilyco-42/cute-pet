@echo off
REM cute-pet 鸿蒙宿主壳构建脚本(本地)
setlocal
set "DE=D:\DevEco\DevEco Studio"
set "DEVECO_SDK_HOME=%DE%\sdk"
set "PATH=%DE%\tools\node;%DE%\tools\ohpm\bin;%DE%\tools\hvigor\bin;%PATH%"
cd /d "%~dp0"
echo [1/3] ohpm install
call "%DE%\tools\ohpm\bin\ohpm.bat" install
echo [2/3] hvigorw version
node "%DE%\tools\hvigor\bin\hvigorw.js" --version
echo [3/3] assembleHap
node "%DE%\tools\hvigor\bin\hvigorw.js" assembleHap --mode module -p product=default -p buildMode=debug --no-daemon
echo EXITCODE=%ERRORLEVEL%
