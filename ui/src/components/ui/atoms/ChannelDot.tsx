import type { ComponentProps } from "react";
import type { Channel } from "../../../api";

export function ChannelDot({
  channel,
  className,
  ...props
}: ComponentProps<"span"> & { channel: Channel }) {
  const classNames = ["channel-dot", channel, className ?? ""].filter(Boolean).join(" ");
  return <span className={classNames} aria-hidden="true" {...props} />;
}
