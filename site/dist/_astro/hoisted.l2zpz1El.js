import"./hoisted.C12hPfrL.js";const R=document.querySelector(".light-rays");function x(i){const e=i.match(/^#?([\da-f]{6})$/i);if(!e)return[1,1,1];const s=parseInt(e[1],16);return[(s>>16&255)/255,(s>>8&255)/255,(s&255)/255]}const t={raysColor:x("#55cae8"),raysSpeed:1,lightSpread:.9,rayLength:1.6,pulsating:0,fadeDistance:1,saturation:.9,mouseInfluence:.08,noiseAmount:.06,distortion:.04},F=`
precision highp float;
uniform float iTime;
uniform vec2  iResolution;
uniform vec2  rayPos;
uniform vec2  rayDir;
uniform vec3  raysColor;
uniform float raysSpeed;
uniform float lightSpread;
uniform float rayLength;
uniform float pulsating;
uniform float fadeDistance;
uniform float saturation;
uniform vec2  mousePos;
uniform float mouseInfluence;
uniform float noiseAmount;
uniform float distortion;

float noise(vec2 st) {
  return fract(sin(dot(st.xy, vec2(12.9898,78.233))) * 43758.5453123);
}

float rayStrength(vec2 raySource, vec2 rayRefDirection, vec2 coord,
                  float seedA, float seedB, float speed) {
  vec2 sourceToCoord = coord - raySource;
  vec2 dirNorm = normalize(sourceToCoord);
  float cosAngle = dot(dirNorm, rayRefDirection);
  float distortedAngle = cosAngle + distortion * sin(iTime * 2.0 + length(sourceToCoord) * 0.01) * 0.2;
  float spreadFactor = pow(max(distortedAngle, 0.0), 1.0 / max(lightSpread, 0.001));
  float dist = length(sourceToCoord);
  float maxDistance = iResolution.x * rayLength;
  float lengthFalloff = clamp((maxDistance - dist) / maxDistance, 0.0, 1.0);
  float fadeFalloff = clamp((iResolution.x * fadeDistance - dist) / (iResolution.x * fadeDistance), 0.5, 1.0);
  float pulse = pulsating > 0.5 ? (0.8 + 0.2 * sin(iTime * speed * 3.0)) : 1.0;
  float baseStrength = clamp(
    (0.45 + 0.15 * sin(distortedAngle * seedA + iTime * speed)) +
    (0.3 + 0.2 * cos(-distortedAngle * seedB + iTime * speed)),
    0.0, 1.0);
  return baseStrength * lengthFalloff * fadeFalloff * spreadFactor * pulse;
}

void main() {
  vec2 fragCoord = gl_FragCoord.xy;
  vec2 coord = vec2(fragCoord.x, iResolution.y - fragCoord.y);

  vec2 finalRayDir = rayDir;
  if (mouseInfluence > 0.0) {
    vec2 mouseScreenPos = mousePos * iResolution.xy;
    vec2 mouseDirection = normalize(mouseScreenPos - rayPos);
    finalRayDir = normalize(mix(rayDir, mouseDirection, mouseInfluence));
  }

  vec4 rays1 = vec4(1.0) * rayStrength(rayPos, finalRayDir, coord, 36.2214, 21.11349, 1.5 * raysSpeed);
  vec4 rays2 = vec4(1.0) * rayStrength(rayPos, finalRayDir, coord, 22.3991, 18.0234, 1.1 * raysSpeed);
  vec4 fragColor = rays1 * 0.5 + rays2 * 0.4;

  if (noiseAmount > 0.0) {
    float n = noise(coord * 0.01 + iTime * 0.1);
    fragColor.rgb *= (1.0 - noiseAmount + noiseAmount * n);
  }

  float brightness = 1.0 - (coord.y / iResolution.y);
  fragColor.x *= 0.1 + brightness * 0.8;
  fragColor.y *= 0.3 + brightness * 0.6;
  fragColor.z *= 0.5 + brightness * 0.5;

  if (saturation != 1.0) {
    float gray = dot(fragColor.rgb, vec3(0.299, 0.587, 0.114));
    fragColor.rgb = mix(vec3(gray), fragColor.rgb, saturation);
  }

  fragColor.rgb *= raysColor;
  gl_FragColor = fragColor;
}`,P=`
attribute vec2 position;
void main() { gl_Position = vec4(position, 0.0, 1.0); }`;function w(i){const e=i.getContext("webgl",{alpha:!0,premultipliedAlpha:!0,antialias:!1});if(!e)return;const s=(r,a)=>{const m=e.createShader(r);return e.shaderSource(m,a),e.compileShader(m),m},n=e.createProgram();if(e.attachShader(n,s(e.VERTEX_SHADER,P)),e.attachShader(n,s(e.FRAGMENT_SHADER,F)),e.linkProgram(n),!e.getProgramParameter(n,e.LINK_STATUS))return;e.useProgram(n);const v=e.createBuffer();e.bindBuffer(e.ARRAY_BUFFER,v),e.bufferData(e.ARRAY_BUFFER,new Float32Array([-1,-1,3,-1,-1,3]),e.STATIC_DRAW);const d=e.getAttribLocation(n,"position");e.enableVertexAttribArray(d),e.vertexAttribPointer(d,2,e.FLOAT,!1,0,0);const o=r=>e.getUniformLocation(n,r),g=o("iTime"),S=o("iResolution"),C=o("rayPos"),b=o("rayDir"),D=o("mousePos");e.uniform3fv(o("raysColor"),t.raysColor),e.uniform1f(o("raysSpeed"),t.raysSpeed),e.uniform1f(o("lightSpread"),t.lightSpread),e.uniform1f(o("rayLength"),t.rayLength),e.uniform1f(o("pulsating"),t.pulsating),e.uniform1f(o("fadeDistance"),t.fadeDistance),e.uniform1f(o("saturation"),t.saturation),e.uniform1f(o("mouseInfluence"),t.mouseInfluence),e.uniform1f(o("noiseAmount"),t.noiseAmount),e.uniform1f(o("distortion"),t.distortion);let l=0,u=0;const y=()=>{const r=Math.min(window.devicePixelRatio||1,2),a=i.getBoundingClientRect();l=Math.max(1,Math.round(a.width*r)),u=Math.max(1,Math.round(a.height*r)),i.width=l,i.height=u,e.viewport(0,0,l,u),e.uniform2f(S,l,u),e.uniform2f(C,l*.5,-u*.2),e.uniform2f(b,0,1)};y(),new ResizeObserver(y).observe(i);let c=[.5,.2],f=[.5,.2];window.addEventListener("pointermove",r=>{const a=i.getBoundingClientRect();c=[(r.clientX-a.left)/a.width,(r.clientY-a.top)/a.height]},{passive:!0});const h=matchMedia("(prefers-reduced-motion: reduce)").matches;let p=!0;new IntersectionObserver(([r])=>{p=r.isIntersecting}).observe(i);const T=performance.now(),A=()=>{p&&(f[0]+=(c[0]-f[0])*.08,f[1]+=(c[1]-f[1])*.08,e.uniform2f(D,f[0],f[1]),e.uniform1f(g,(performance.now()-T)*.001),e.drawArrays(e.TRIANGLES,0,3)),h||requestAnimationFrame(A)};h?(e.uniform1f(g,12),e.drawArrays(e.TRIANGLES,0,3)):requestAnimationFrame(A)}R&&w(R);
