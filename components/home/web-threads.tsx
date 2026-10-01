"use client";

// Adapted from WebThreads by React Bits (https://reactbits.dev), licensed MIT +
// Commons Clause. Site code only: it is not part of the audiocn registry.

import { Mesh, Program, Renderer, Triangle } from "ogl";
import { useEffect, useRef } from "react";

import { cn } from "@/lib/utils";

export type FanMode = "center" | "left" | "right";

export interface WebThreadsProps {
  color1?: string;
  color2?: string;
  color3?: string;
  speed?: number;
  threadCount?: number;
  frequency?: number;
  spread?: number;
  taper?: number;
  position?: number;
  fanMode?: FanMode;
  glow?: number;
  falloff?: number;
  thickness?: number;
  brightness?: number;
  opacity?: number;
  mirror?: boolean;
  shimmer?: boolean;
  grain?: boolean;
  grainIntensity?: number;
  mouseInteraction?: boolean;
  mouseStrength?: number;
  backgroundColor?: string;
  lightMode?: boolean;
  className?: string;
}

interface Uniform<T> {
  value: T;
}

interface Uniforms {
  iTime: Uniform<number>;
  iResolution: Uniform<Float32Array>;
  uSpeed: Uniform<number>;
  uThreadCount: Uniform<number>;
  uFrequency: Uniform<number>;
  uSpread: Uniform<number>;
  uTaper: Uniform<number>;
  uPosition: Uniform<number>;
  uFanMode: Uniform<number>;
  uGlow: Uniform<number>;
  uFalloff: Uniform<number>;
  uThickness: Uniform<number>;
  uBrightness: Uniform<number>;
  uOpacity: Uniform<number>;
  uMirror: Uniform<number>;
  uShimmer: Uniform<number>;
  uGrain: Uniform<number>;
  uGrainIntensity: Uniform<number>;
  uColor1: Uniform<Float32Array>;
  uColor2: Uniform<Float32Array>;
  uColor3: Uniform<Float32Array>;
  uBackgroundColor: Uniform<Float32Array>;
  uLightMode: Uniform<boolean>;
  uMouse: Uniform<Float32Array>;
  uMouseStrength: Uniform<number>;
  uEnableMouse: Uniform<number>;
  uMouseActive: Uniform<number>;
}

const HEX_COLOR = /^#?[\da-f]{6}$/iu;
const CHANNEL_MAX = 255;
const MAX_DPR = 2;
const MS_PER_SECOND = 1000;
/** How much of the gap to the cursor the pinch point closes each frame. */
const MOUSE_EASING = 0.05;

const FAN_MODE: Record<FanMode, number> = { center: 0, left: 1, right: 2 };

const channel = (digits: string, offset: number) =>
  Number.parseInt(digits.slice(offset, offset + 2), 16) / CHANNEL_MAX;

/** Writes a hex colour into a uniform as 0..1 RGB; white if it cannot parse. */
const writeColor = (target: Float32Array, hex: string) => {
  const digits = HEX_COLOR.test(hex) ? hex.replace("#", "") : "ffffff";
  target[0] = channel(digits, 0);
  target[1] = channel(digits, 2);
  target[2] = channel(digits, 4);
};

const createUniforms = (): Uniforms => ({
  iResolution: { value: new Float32Array([1, 1]) },
  iTime: { value: 0 },
  uBackgroundColor: { value: new Float32Array([1, 1, 1]) },
  uBrightness: { value: 0.6 },
  uColor1: { value: new Float32Array([1, 1, 1]) },
  uColor2: { value: new Float32Array([1, 1, 1]) },
  uColor3: { value: new Float32Array([1, 1, 1]) },
  uEnableMouse: { value: 1 },
  uFalloff: { value: 0.6 },
  uFanMode: { value: 0 },
  uFrequency: { value: 5 },
  uGlow: { value: 0.02 },
  uGrain: { value: 1 },
  uGrainIntensity: { value: 0.05 },
  uLightMode: { value: false },
  uMirror: { value: 1 },
  uMouse: { value: new Float32Array([0.5, 0.5]) },
  uMouseActive: { value: 0 },
  uMouseStrength: { value: 0.3 },
  uOpacity: { value: 1 },
  uPosition: { value: 0.5 },
  uShimmer: { value: 0 },
  uSpeed: { value: 0.2 },
  uSpread: { value: 0.18 },
  uTaper: { value: 1 },
  uThickness: { value: 1.1 },
  uThreadCount: { value: 6 },
});

const vertex = `#version 300 es
in vec2 position;
void main() {
  gl_Position = vec4(position, 0.0, 1.0);
}
`;

const fragment = `#version 300 es
precision highp float;
uniform vec2 iResolution;
uniform float iTime;
uniform float uSpeed;
uniform float uThreadCount;
uniform float uFrequency;
uniform float uSpread;
uniform float uTaper;
uniform float uPosition;
uniform float uFanMode;
uniform float uGlow;
uniform float uFalloff;
uniform float uThickness;
uniform float uBrightness;
uniform float uOpacity;
uniform float uMirror;
uniform float uShimmer;
uniform float uGrain;
uniform float uGrainIntensity;
uniform vec3 uColor1;
uniform vec3 uColor2;
uniform vec3 uColor3;
uniform vec3 uBackgroundColor;
uniform bool uLightMode;
uniform vec2 uMouse;
uniform float uMouseStrength;
uniform float uEnableMouse;
uniform float uMouseActive;
out vec4 fragColor;

#define TAU 6.28318530718
#define MAX_THREADS 10

float glow(float x, float str, float dist) {
  return dist / pow(max(x, 1e-4), str);
}

void main() {
  vec2 uv = gl_FragCoord.xy / iResolution.xy;
  float n = max(uThreadCount, 1.0);

  float pinchX = uFanMode < 0.5 ? 0.5 : (uFanMode < 1.5 ? 0.0 : 1.0);
  if (uEnableMouse > 0.5) {
    pinchX = mix(pinchX, uMouse.x, clamp(uMouseStrength, 0.0, 1.0) * uMouseActive);
  }

  float spreadDx = uSpread * abs(uv.x - pinchX);
  float baseT = iTime * uSpeed;
  float tauOverN = TAU / n;
  float mirror = uMirror > 0.5 ? sign(pinchX - uv.x) : 1.0;
  bool doShimmer = uShimmer > 0.5;
  float shimmerT = iTime * 1.7;
  float invThickness = 1.0 / max(uThickness, 0.01);
  float xFreq = uv.x * uFrequency;
  float yOff = uv.y - uPosition;
  float ciScale = n > 1.0 ? 1.0 / (n - 1.0) : 0.0;

  vec3 col = vec3(0.0);
  float gsum = 0.0;

  for (int idx = 0; idx < MAX_THREADS; idx++) {
    float i = float(idx);
    if (i >= n) break;

    float amplitude = spreadDx * (1.0 + i * uTaper);
    float shimmer = doShimmer ? sin(shimmerT + i * 1.3) * 0.35 : 0.0;
    float phase = (baseT + i * tauOverN) * mirror + shimmer;

    float sdf = abs(yOff + sin(xFreq + phase) * amplitude) * invThickness;

    float g = glow(sdf, uFalloff, uGlow);
    float ci = i * ciScale;
    vec3 threadCol = mix(uColor1, uColor2, ci);

    col += g * threadCol;
    gsum += g;
  }

  float coreAmt = smoothstep(0.5, 2.2, gsum);
  col = mix(col, uColor3 * gsum, coreAmt * 0.5);

  float bright = uBrightness;
  if (uEnableMouse > 0.5) {
    vec2 md = uv - uMouse;
    float d2 = dot(md, md);
    bright += clamp(uMouseStrength, 0.0, 1.0) * uMouseActive * exp(-d2 * 6.0) * 0.6;
  }
  col *= bright;

  float alpha = clamp(gsum, 0.0, 1.0) * uOpacity;

  vec3 outRgb = col * alpha;

  if (uGrain > 0.5) {
    float gv = (fract(sin(dot(gl_FragCoord.xy, vec2(12.9898, 78.233)) + iTime) * 43758.5453) - 0.5) * uGrainIntensity;
    outRgb = clamp(outRgb + gv, 0.0, 1.0);
    alpha = clamp(alpha + gv, 0.0, 1.0);
  }

  if (uLightMode) {
    vec3 mapped = vec3(1.0) - exp(-max(col, vec3(0.0)) * 1.3);
    float rawEnergy = clamp(max(mapped.r, max(mapped.g, mapped.b)) * uOpacity, 0.0, 1.0);
    float coverage = smoothstep(0.18, 0.72, rawEnergy);
    coverage *= coverage;
    vec3 hue = mapped / max(max(mapped.r, max(mapped.g, mapped.b)), 1e-4);
    vec3 chroma = pow(clamp(hue, 0.0, 1.0), vec3(0.78));
    vec3 pigment = mix(chroma, vec3(0.08), 0.12);
    vec3 ink = mix(vec3(0.9), pigment, 0.82 + coverage * 0.18);
    fragColor = vec4(mix(uBackgroundColor, ink, coverage), 1.0);
  } else {
    fragColor = vec4(outRgb, alpha);
  }
}
`;

const DEFAULTS: Required<Omit<WebThreadsProps, "className">> = {
  backgroundColor: "#FFFFFF",
  brightness: 0.6,
  color1: "#5227FF",
  color2: "#FF9FFC",
  color3: "#FFFFFF",
  falloff: 0.6,
  fanMode: "center",
  frequency: 5,
  glow: 0.02,
  grain: true,
  grainIntensity: 0.05,
  lightMode: false,
  mirror: true,
  mouseInteraction: true,
  mouseStrength: 0.3,
  opacity: 1,
  position: 0.5,
  shimmer: false,
  speed: 0.2,
  spread: 0.18,
  taper: 1,
  thickness: 1.1,
  threadCount: 6,
};

/** Drops props passed as `undefined`, so they can't override a default. */
const definedProps = <T extends object>(props: T) =>
  Object.fromEntries(
    Object.entries(props).filter(([, value]) => value !== undefined)
  ) as Partial<T>;

/** Glowing threads woven along a sine wave, drawn with WebGL. */
export const WebThreads = ({ className, ...props }: WebThreadsProps) => {
  const {
    backgroundColor,
    brightness,
    color1,
    color2,
    color3,
    falloff,
    fanMode,
    frequency,
    glow,
    grain,
    grainIntensity,
    lightMode,
    mirror,
    mouseInteraction,
    mouseStrength,
    opacity,
    position,
    shimmer,
    speed,
    spread,
    taper,
    thickness,
    threadCount,
  } = { ...DEFAULTS, ...definedProps(props) };
  const containerRef = useRef<HTMLDivElement>(null);
  const uniformsRef = useRef<Uniforms | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    const renderer = new Renderer({
      alpha: true,
      antialias: false,
      dpr: Math.min(window.devicePixelRatio || 1, MAX_DPR),
      premultipliedAlpha: true,
      webgl: 2,
    });
    const { gl } = renderer;
    gl.clearColor(0, 0, 0, 0);
    const { canvas } = gl;
    Object.assign(canvas.style, {
      display: "block",
      height: "100%",
      width: "100%",
    });
    container.append(canvas);

    const uniforms = createUniforms();
    uniformsRef.current = uniforms;
    const mesh = new Mesh(gl, {
      geometry: new Triangle(gl),
      program: new Program(gl, { fragment, uniforms, vertex }),
    });

    const setSize = () => {
      const rect = container.getBoundingClientRect();
      renderer.setSize(
        Math.max(1, Math.floor(rect.width)),
        Math.max(1, Math.floor(rect.height))
      );
      uniforms.iResolution.value[0] = gl.drawingBufferWidth;
      uniforms.iResolution.value[1] = gl.drawingBufferHeight;
      renderer.render({ scene: mesh });
    };
    const resizeObserver = new ResizeObserver(setSize);
    resizeObserver.observe(container);
    setSize();

    let mouseX = 0.5;
    let mouseY = 0.5;
    let targetX = 0.5;
    let targetY = 0.5;
    let currentActive = 0;
    let targetActive = 0;

    const onMouseMove = (event: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      targetX = (event.clientX - rect.left) / rect.width;
      targetY = 1 - (event.clientY - rect.top) / rect.height;
      targetActive = 1;
    };
    const onMouseEnter = () => {
      targetActive = 1;
    };
    const onMouseLeave = () => {
      targetActive = 0;
    };
    canvas.addEventListener("mousemove", onMouseMove);
    canvas.addEventListener("mouseenter", onMouseEnter);
    canvas.addEventListener("mouseleave", onMouseLeave);

    let frame = 0;
    let onScreen = true;
    const startedAt = performance.now();

    const loop = (now: number) => {
      uniforms.iTime.value = (now - startedAt) / MS_PER_SECOND;
      mouseX += MOUSE_EASING * (targetX - mouseX);
      mouseY += MOUSE_EASING * (targetY - mouseY);
      currentActive += MOUSE_EASING * (targetActive - currentActive);
      uniforms.uMouse.value[0] = mouseX;
      uniforms.uMouse.value[1] = mouseY;
      uniforms.uMouseActive.value = currentActive;
      renderer.render({ scene: mesh });
      frame = requestAnimationFrame(loop);
    };

    // Runs only while the threads are on screen and the tab is visible.
    const update = () => {
      const running = frame !== 0;
      const shouldRun = onScreen && !document.hidden;
      if (shouldRun && !running) {
        frame = requestAnimationFrame(loop);
      } else if (!shouldRun && running) {
        cancelAnimationFrame(frame);
        frame = 0;
      }
    };

    const intersectionObserver = new IntersectionObserver((entries) => {
      onScreen = entries.at(-1)?.isIntersecting ?? false;
      update();
    });
    intersectionObserver.observe(container);
    document.addEventListener("visibilitychange", update);
    update();

    return () => {
      cancelAnimationFrame(frame);
      frame = 0;
      resizeObserver.disconnect();
      intersectionObserver.disconnect();
      document.removeEventListener("visibilitychange", update);
      canvas.removeEventListener("mousemove", onMouseMove);
      canvas.removeEventListener("mouseenter", onMouseEnter);
      canvas.removeEventListener("mouseleave", onMouseLeave);
      uniformsRef.current = null;
      canvas.remove();
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    };
  }, []);

  useEffect(() => {
    const uniforms = uniformsRef.current;
    if (!uniforms) {
      return;
    }
    uniforms.uSpeed.value = speed;
    uniforms.uThreadCount.value = Math.round(threadCount);
    uniforms.uFrequency.value = frequency;
    uniforms.uSpread.value = spread;
    uniforms.uTaper.value = taper;
    uniforms.uPosition.value = position;
    uniforms.uFanMode.value = FAN_MODE[fanMode] ?? 0;
    uniforms.uGlow.value = glow;
    uniforms.uFalloff.value = falloff;
    uniforms.uThickness.value = thickness;
    uniforms.uBrightness.value = brightness;
    uniforms.uOpacity.value = opacity;
    uniforms.uMirror.value = mirror ? 1 : 0;
    uniforms.uShimmer.value = shimmer ? 1 : 0;
    uniforms.uGrain.value = grain ? 1 : 0;
    uniforms.uGrainIntensity.value = grainIntensity;
    writeColor(uniforms.uColor1.value, color1);
    writeColor(uniforms.uColor2.value, color2);
    writeColor(uniforms.uColor3.value, color3);
    writeColor(uniforms.uBackgroundColor.value, backgroundColor);
    uniforms.uLightMode.value = lightMode;
    uniforms.uMouseStrength.value = mouseStrength;
    uniforms.uEnableMouse.value = mouseInteraction ? 1 : 0;
  }, [
    color1,
    color2,
    color3,
    speed,
    threadCount,
    frequency,
    spread,
    taper,
    position,
    fanMode,
    glow,
    falloff,
    thickness,
    brightness,
    opacity,
    mirror,
    shimmer,
    grain,
    grainIntensity,
    mouseInteraction,
    mouseStrength,
    backgroundColor,
    lightMode,
  ]);

  return (
    <div
      className={cn("relative h-full w-full overflow-hidden", className)}
      ref={containerRef}
    />
  );
};
