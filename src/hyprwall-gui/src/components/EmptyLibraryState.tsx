import { CenteredMessage } from "./CenteredMessage";

const COLOR = "#555";

/** Plain line-drawing frown, matching the app's other custom SVG icons
 * (cog, chevrons) rather than a colorful emoji -- a colored emoji glyph
 * can't be recolored via CSS, which would break "same color as the text". */
function SadFaceIcon() {
  return (
    <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke={COLOR} strokeWidth="1.5">
      <circle cx="12" cy="12" r="10" />
      <circle cx="8.5" cy="10" r="1" fill={COLOR} stroke="none" />
      <circle cx="15.5" cy="10" r="1" fill={COLOR} stroke="none" />
      <path d="M8 16.5c1-1.5 2.5-2.2 4-2.2s3 .7 4 2.2" strokeLinecap="round" />
    </svg>
  );
}

export function EmptyLibraryState() {
  return (
    <CenteredMessage icon={<SadFaceIcon />} color={COLOR}>
      Oh... oh... you haven't configured an Image Library Folder yet. Click the Cog icon on the top
      right and select the System category to configure a path.
    </CenteredMessage>
  );
}
