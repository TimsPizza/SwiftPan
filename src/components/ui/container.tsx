import { cn } from "@/lib/utils"; // 没有就用 (…classes:string[])=>classes.filter(Boolean).join(" ")
import * as React from "react";

type Ripple = { id: number; x: number; y: number; size: number };

type RippleContainerProps = {
  children: React.ReactNode;
  /** 是否限制在容器内（Material 默认 bounded=true） */
  bounded?: boolean;
  /** 是否强制从中心扩散（如 IconButton 的视觉风格） */
  center?: boolean;
  /** 颜色，默认 currentColor（配合 text-* 可变色） */
  color?: string;
  /** 最高不透明度（0~1），默认 0.2 */
  opacity?: number;
  /** 动画时长（毫秒），默认 550 */
  durationMs?: number;
  className?: string;
} & React.HTMLAttributes<HTMLDivElement>;

export function RippleContainer({
  children,
  bounded = true,
  center = false,
  color = "currentColor",
  opacity = 0.2,
  durationMs = 550,
  className,
  ...rest
}: RippleContainerProps) {
  const hostRef = React.useRef<HTMLDivElement>(null);
  const [ripples, setRipples] = React.useState<Ripple[]>([]);
  const nextId = React.useRef(1);

  const createRipple = React.useCallback(
    (e: React.PointerEvent) => {
      const host = hostRef.current;
      if (!host) return;

      const rect = host.getBoundingClientRect();
      const localX = e.clientX - rect.left;
      const localY = e.clientY - rect.top;

      // 计算能覆盖容器的半径（对角线）
      const maxDX = Math.max(localX, rect.width - localX);
      const maxDY = Math.max(localY, rect.height - localY);
      const radius = Math.hypot(maxDX, maxDY);
      const size = radius * 2;

      const x = center ? rect.width / 2 : localX;
      const y = center ? rect.height / 2 : localY;

      const id = nextId.current++;
      setRipples((prev) => [...prev, { id, x, y, size }]);

      // 动画结束后移除（+30ms 保险）
      window.setTimeout(() => {
        setRipples((prev) => prev.filter((r) => r.id !== id));
      }, durationMs + 30);
    },
    [center, durationMs],
  );

  return (
    <div
      ref={hostRef}
      className={cn(
        "relative touch-manipulation select-none",
        bounded ? "overflow-hidden" : "overflow-visible",
        className,
      )}
      // 禁止 iOS 长按系统菜单/灰色高亮，保证反馈稳定
      onContextMenu={(e) => e.preventDefault()}
      onPointerDown={(e) => {
        // 只对主指针触发（避免多指/笔侧键等奇怪情况）
        if (e.isPrimary) createRipple(e);
        rest.onPointerDown?.(e);
      }}
      style={{ WebkitTapHighlightColor: "transparent" }}
      {...rest}
    >
      {/* 你的内容 */}
      {children}

      {/* 水波纹层（独立图层避免抖动） */}
      <div className="pointer-events-none absolute inset-0 will-change-transform">
        {ripples.map((r) => (
          <span
            key={r.id}
            className="absolute rounded-full"
            style={{
              // 让圆心落在点击点：左上角减半径
              left: `${r.x - r.size / 2}px`,
              top: `${r.y - r.size / 2}px`,
              width: `${r.size}px`,
              height: `${r.size}px`,
              backgroundColor: color,
              opacity,
              transform: "scale(0)",
              // 独立合成层让动画丝滑
              willChange: "transform, opacity",
              // 动画：从 0 放到 1，同时淡出
              animation: `ripple ${durationMs}ms ease-out forwards`,
              // 无边界时可以让涟漪外溢，但通常我们限制在容器内
              filter: "blur(0.2px)", // 轻微抗锯齿
            }}
          />
        ))}
      </div>

      {/* 关键帧（就地注册一次，不想改 Tailwind 配置的话最省事） */}
      <style>{`
        @keyframes ripple {
          0%   { transform: scale(0);   opacity: var(--ripple-opacity-start, ${opacity}); }
          75%  { transform: scale(1);   opacity: calc(var(--ripple-opacity-start, ${opacity}) * 0.6); }
          100% { transform: scale(1.05); opacity: 0; }
        }
      `}</style>
    </div>
  );
}
