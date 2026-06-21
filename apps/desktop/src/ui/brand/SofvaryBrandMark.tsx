interface SofvaryBrandMarkProps {
  className?: string;
}

export function SofvaryBrandMark({ className }: SofvaryBrandMarkProps) {
  return <img className={className} src="/brand/sofvary-mark.png" alt="" aria-hidden="true" draggable={false} />;
}
