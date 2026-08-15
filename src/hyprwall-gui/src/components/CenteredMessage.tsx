import type { ReactNode } from "react";

interface Props {
  icon: ReactNode;
  color: string;
  children: ReactNode;
}

/** Shared layout for whole-page "nothing to show here, here's why" states
 * (empty library, blocking errors): an icon, then a message below it, both
 * centered in the available space. */
export function CenteredMessage({ icon, color, children }: Props) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 14,
        height: "100%",
        padding: "0 20px",
        textAlign: "center",
      }}
    >
      {icon}
      <p style={{ color, fontSize: 13, maxWidth: 360, margin: 0 }}>{children}</p>
    </div>
  );
}
