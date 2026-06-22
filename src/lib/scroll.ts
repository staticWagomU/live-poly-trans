export const AUTO_SCROLL_THRESHOLD_PX = 48;

type ScrollPosition = {
  scrollTop: number;
  clientHeight: number;
  scrollHeight: number;
};

export function isScrolledToBottom(position: ScrollPosition) {
  return remainingScrollDistance(position) <= AUTO_SCROLL_THRESHOLD_PX;
}

export function remainingScrollDistance({ scrollTop, clientHeight, scrollHeight }: ScrollPosition) {
  return scrollHeight - clientHeight - scrollTop;
}
