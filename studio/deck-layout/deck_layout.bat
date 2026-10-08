@echo off
rem Double-click to open the Stream Deck layout editor.
python "%~dp0deck_layout.py" %*
if errorlevel 1 pause
