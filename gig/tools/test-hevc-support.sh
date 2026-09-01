#!/usr/bin/env bash
# Does Chromium on THIS Pi actually decode HEVC? Answer before transcoding 28
# files. Chromium on Raspberry Pi OS often ships without HEVC (licensed codec),
# in which case transcoding does not speed clips up -- it stops them playing.
#
# Run ON THE PI:  ./gig/tools/test-hevc-support.sh
set -euo pipefail
cd "$(dirname "$0")/.."

SRC="$(ls media/h264/*.mp4 | head -1)"
mkdir -p media/hevc
OUT="media/hevc/_hevc_probe.mp4"

if [ ! -f "$OUT" ]; then
  echo "== transcoding ONE clip to HEVC (30s sample)"
  ffmpeg -y -v error -t 30 -i "$SRC" \
    -c:v libx265 -preset medium -crf 24 -tag:v hvc1 \
    -pix_fmt yuv420p -an "$OUT"
fi
echo "== sample ready: $OUT"
ffprobe -v error -select_streams v:0 \
  -show_entries stream=codec_name,width,height -of default=nw=1 "$OUT"

cat > probe/hevc-test.html <<'HTML'
<!doctype html><meta charset=utf-8><title>HEVC support</title>
<body style="background:#000;color:#0f0;font:16px monospace">
<pre id=o>testing...</pre>
<video id=v src="/gig/media/hevc/_hevc_probe.mp4" autoplay muted playsinline
       style="width:640px"></video>
<script>
var o=document.getElementById('o'), v=document.getElementById('v'), lines=[];
function say(s){lines.push(s);o.textContent=lines.join('\n');
  try{fetch('/collect?name=hevc',{method:'POST',body:lines.join('\n')})}catch(e){}}
say('canPlayType hvc1 : "'+v.canPlayType('video/mp4; codecs="hvc1"')+'"');
say('canPlayType hev1 : "'+v.canPlayType('video/mp4; codecs="hev1"')+'"');
say('canPlayType avc1 : "'+v.canPlayType('video/mp4; codecs="avc1.640028"')+'"');
v.addEventListener('error',function(){say('VIDEO ERROR: HEVC NOT DECODABLE')});
v.addEventListener('loadeddata',function(){say('loadeddata ok '+v.videoWidth+'x'+v.videoHeight)});
var n=0;
setInterval(function(){
  if(v.currentTime>0 && !v.paused){ n++;
    if(n===3) say('PLAYING at t='+v.currentTime.toFixed(1)+'  => HEVC WORKS'); }
},1000);
setTimeout(function(){ if(n===0) say('NEVER ADVANCED => HEVC NOT USABLE'); },8000);
</script>
HTML

echo "== now run:  ./gig/deploy/run-probe.sh hevc-test.html"
echo "   then:     cat ~/vj-results/hevc.log"
