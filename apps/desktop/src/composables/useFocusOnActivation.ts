import { nextTick, watch, type Ref } from "vue";
import { withComponentEpoch } from "./useComponentEpoch";

type FocusableElement = {
  focus: () => void;
  select?: () => void;
};

export function useFocusOnActivation<T extends FocusableElement>(
  target: Ref<T | null>,
  active: () => boolean,
  select = false,
) {
  const focusEpoch = withComponentEpoch();

  watch(
    active,
    (isActive) => {
      const epoch = focusEpoch.nextEpoch();
      if (!isActive) return;

      void nextTick(() => {
        if (!focusEpoch.assertAlive(epoch) || !active()) return;
        const element = target.value;
        element?.focus();
        if (select) element?.select?.();
      });
    },
  );
}
