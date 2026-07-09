// Inline SVG icons (no icon dependency). Stroke-based, inherit currentColor.
import type { ReactNode, SVGProps } from "react";

type P = SVGProps<SVGSVGElement> & { size?: number };
function svg(path: ReactNode) {
  return function Icon({ size = 18, ...rest }: P) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.7}
        strokeLinecap="round"
        strokeLinejoin="round"
        {...rest}
      >
        {path}
      </svg>
    );
  };
}

export const IconBucket = svg(
  <>
    <path d="M4 7h16l-1.5 12.5a2 2 0 0 1-2 1.5H7.5a2 2 0 0 1-2-1.5L4 7Z" />
    <path d="M3 7h18" />
    <path d="M9 4h6" />
  </>,
);
export const IconFolder = svg(
  <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z" />,
);
export const IconFile = svg(
  <>
    <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8l-5-5Z" />
    <path d="M14 3v5h5" />
  </>,
);
export const IconImage = svg(
  <>
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <circle cx="8.5" cy="9" r="1.5" />
    <path d="m4 16 4.5-4 3 2.5L15 11l5 5" />
  </>,
);
export const IconCode = svg(
  <>
    <path d="m9 8-4 4 4 4" />
    <path d="m15 8 4 4-4 4" />
  </>,
);
export const IconData = svg(
  <>
    <ellipse cx="12" cy="6" rx="7" ry="3" />
    <path d="M5 6v12c0 1.7 3.1 3 7 3s7-1.3 7-3V6" />
    <path d="M5 12c0 1.7 3.1 3 7 3s7-1.3 7-3" />
  </>,
);
export const IconModel = svg(
  <>
    <circle cx="12" cy="5" r="2" />
    <circle cx="5" cy="18" r="2" />
    <circle cx="19" cy="18" r="2" />
    <path d="M12 7v4m0 0-5 5m5-5 5 5" />
  </>,
);
export const IconDoc = svg(
  <>
    <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8l-5-5Z" />
    <path d="M14 3v5h5M8 13h8M8 17h5" />
  </>,
);
export const IconArchive = svg(
  <>
    <rect x="3" y="4" width="18" height="4" rx="1" />
    <path d="M5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8" />
    <path d="M10 12h4" />
  </>,
);
export const IconUpload = svg(
  <>
    <path d="M12 15V4m0 0-4 4m4-4 4 4" />
    <path d="M4 17v2a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-2" />
  </>,
);
export const IconDownload = svg(
  <>
    <path d="M12 4v11m0 0-4-4m4 4 4-4" />
    <path d="M4 17v2a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-2" />
  </>,
);
export const IconCopy = svg(
  <>
    <rect x="9" y="9" width="12" height="12" rx="2" />
    <path d="M5 15V5a2 2 0 0 1 2-2h8" />
  </>,
);
export const IconTrash = svg(
  <>
    <path d="M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2" />
    <path d="M6 7v12a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V7M10 11v6M14 11v6" />
  </>,
);
export const IconSearch = svg(
  <>
    <circle cx="11" cy="11" r="7" />
    <path d="m21 21-4.3-4.3" />
  </>,
);
export const IconGear = svg(
  <>
    <circle cx="12" cy="12" r="3" />
    <path d="M12 2v3m0 14v3M2 12h3m14 0h3m-3.5-7.5-2 2m-11 11 2-2m0-11-2-2m13 15-2-2" />
  </>,
);
export const IconPlus = svg(<path d="M12 5v14M5 12h14" />);
export const IconClose = svg(<path d="m6 6 12 12M18 6 6 18" />);
export const IconCheck = svg(<path d="m5 12 5 5L20 7" />);
export const IconChevron = svg(<path d="m9 6 6 6-6 6" />);
export const IconRefresh = svg(
  <>
    <path d="M20 11a8 8 0 1 0-.6 4" />
    <path d="M20 5v6h-6" />
  </>,
);
export const IconLink = svg(
  <>
    <path d="M10 13a5 5 0 0 0 7 0l2-2a5 5 0 0 0-7-7l-1 1" />
    <path d="M14 11a5 5 0 0 0-7 0l-2 2a5 5 0 0 0 7 7l1-1" />
  </>,
);
export const IconPin = svg(
  <>
    <path d="M12 17v5" />
    <path d="M9 3h6l-1 6 3 3H7l3-3-1-6Z" />
  </>,
);
export const IconLogo = svg(
  <>
    <circle cx="12" cy="12" r="9" strokeWidth={1.6} />
    <circle cx="12" cy="12" r="2.6" fill="currentColor" stroke="none" />
    <circle cx="12" cy="3.5" r="1.6" fill="currentColor" stroke="none" />
    <circle cx="20" cy="16.5" r="1.6" fill="currentColor" stroke="none" />
    <circle cx="4" cy="16.5" r="1.6" fill="currentColor" stroke="none" />
  </>,
);
