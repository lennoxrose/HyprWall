import { CenteredMessage } from "./CenteredMessage";

const COLOR = "var(--hw-text-muted)";

function WarningTriangleIcon() {
  return (
    <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke={COLOR} strokeWidth="1.5">
      <path d="M12 3 L22 20 L2 20 Z" strokeLinejoin="round" strokeLinecap="round" />
      <line x1="12" y1="9.5" x2="12" y2="14.5" strokeLinecap="round" />
      <circle cx="12" cy="17.3" r="0.75" fill={COLOR} stroke="none" />
    </svg>
  );
}

interface Props {
  message: string;
}

/** Whole-page treatment for blocking errors (e.g. the daemon being
 * unreachable) -- same big-icon-then-message layout as EmptyLibraryState,
 * just a warning triangle instead of a frown. */
export function ErrorState({ message }: Props) {
  return (
    <CenteredMessage icon={<WarningTriangleIcon />} color={COLOR}>
      Oh... oh... {message}
    </CenteredMessage>
  );
}
