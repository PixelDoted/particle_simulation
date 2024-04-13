#!/bin/bash

read -p "Frame size (WxH): " SIZE
read -p "Enter fps: " FRAMERATE
read -p "Enter format (e.g. \"rgba\", \"bgra\"): " FORMAT

ffmpeg -f rawvideo -pix_fmt "$FORMAT" -s:v "$SIZE" -r "$FRAMERATE" -i "./frame_buffer.bin" "output.mp4"
echo "Finished converting."
