import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import ImageViewer from "../src/components/chat/ImageViewer.vue";

interface Size {
  width: number;
  height: number;
}

function setStageSize(container: HTMLElement, { width, height }: Size) {
  const stage = container.querySelector(".image-viewer__stage") as HTMLElement | null;
  if (!stage) throw new Error("image viewer stage not found");
  Object.defineProperty(stage, "clientWidth", { configurable: true, value: width });
  Object.defineProperty(stage, "clientHeight", { configurable: true, value: height });
  stage.getBoundingClientRect = () => ({
    x: 0,
    y: 0,
    top: 0,
    left: 0,
    right: width,
    bottom: height,
    width,
    height,
    toJSON: () => ({}),
  });
}

function setNaturalSize(image: HTMLImageElement, { width, height }: Size) {
  Object.defineProperty(image, "naturalWidth", { configurable: true, value: width });
  Object.defineProperty(image, "naturalHeight", { configurable: true, value: height });
}

function transformValues(image: HTMLElement) {
  const match = image.style.transform.match(
    /translate3d\((-?\d+(?:\.\d+)?)px, (-?\d+(?:\.\d+)?)px, 0\) scale\((-?\d+(?:\.\d+)?)\)/
  );
  if (!match) throw new Error(`unexpected transform: ${image.style.transform}`);
  return {
    x: Number(match[1]),
    y: Number(match[2]),
    scale: Number(match[3]),
  };
}

function expectTransform(
  image: HTMLElement,
  expected: { x?: number; y?: number; scale?: number },
) {
  const actual = transformValues(image);
  if (expected.x !== undefined) expect(actual.x).toBeCloseTo(expected.x, 3);
  if (expected.y !== undefined) expect(actual.y).toBeCloseTo(expected.y, 3);
  if (expected.scale !== undefined) expect(actual.scale).toBeCloseTo(expected.scale, 3);
}

async function renderLoadedViewer({
  name,
  natural,
  stage = { width: 800, height: 600 },
  src = `asset://${name}`,
}: {
  name: string;
  natural: Size;
  stage?: Size;
  src?: string;
}) {
  const view = render(ImageViewer, {
    props: {
      image: {
        src,
        name,
      },
    },
  });
  const dialog = view.container.querySelector('[role="dialog"]') as HTMLElement | null;
  const image = view.container.querySelector("img") as HTMLImageElement | null;
  if (!dialog || !image) throw new Error("image viewer was not rendered");
  image.setPointerCapture = () => {};
  image.releasePointerCapture = () => {};
  setStageSize(view.container, stage);
  setNaturalSize(image, natural);
  await fireEvent.load(image);
  return { view, dialog, image };
}

async function dragImage(
  image: HTMLElement,
  from: { x: number; y: number },
  to: { x: number; y: number },
  pointerId = 1,
) {
  await fireEvent.pointerDown(image, {
    button: 0,
    pointerId,
    clientX: from.x,
    clientY: from.y,
  });
  await fireEvent.pointerMove(image, {
    pointerId,
    clientX: to.x,
    clientY: to.y,
  });
}

describe("ImageViewer", () => {
  it("展示图片并在点击遮罩时关闭", async () => {
    const view = render(ImageViewer, {
      props: {
        image: {
          src: "asset://shot.png",
          name: "图片 1.png",
          path: "C:\\shot.png",
          mime: "image/png",
          size: 1536,
        },
      },
    });

    const dialog = view.getByRole("dialog", { name: "图片查看器" });
    const image = view.getByRole("img", { name: "图片 1.png" });
    expect(image).toHaveAttribute("src", "asset://shot.png");

    await fireEvent.click(image);
    expect(view.emitted("close")).toBeUndefined();

    await fireEvent.click(dialog);
    expect(view.emitted("close")).toHaveLength(1);
  });

  it("图片加载后显示宽高、格式和文件大小", async () => {
    const view = render(ImageViewer, {
      props: {
        image: {
          src: "asset://shot.png",
          name: "图片 1.png",
          path: "C:\\shot.png",
          mime: "image/png",
          size: 1536,
        },
      },
    });
    const image = view.getByRole("img", { name: "图片 1.png" }) as HTMLImageElement;
    setNaturalSize(image, { width: 640, height: 480 });

    await fireEvent.load(image);

    expect(view.getByText("图片 1.png")).toBeInTheDocument();
    expect(view.getByText("640 x 480 · PNG · 1.5 KB")).toBeInTheDocument();
  });

  it("大图加载后按 stage 尺寸自动缩小，小图保持原始比例", async () => {
    const large = await renderLoadedViewer({
      name: "large.png",
      natural: { width: 1200, height: 1800 },
    });
    expect(large.image.style.width).toBe("400px");
    expect(large.image.style.height).toBe("600px");
    expectTransform(large.image, { scale: 1 });

    const small = await renderLoadedViewer({
      name: "small.png",
      natural: { width: 320, height: 240 },
    });
    expect(small.image.style.width).toBe("320px");
    expect(small.image.style.height).toBe("240px");
    expectTransform(small.image, { scale: 1 });
  });

  it("自动缩小的大图需用户放大后才可拖动", async () => {
    const { dialog, image } = await renderLoadedViewer({
      name: "large.png",
      natural: { width: 1200, height: 1800 },
    });

    await dragImage(image, { x: 10, y: 20 }, { x: 25, y: 35 });
    expectTransform(image, { x: 0, y: 0, scale: 1 });

    await fireEvent.wheel(dialog, { deltaY: -400 });
    expect(image.style.cursor).toBe("grab");
    await dragImage(image, { x: 10, y: 20 }, { x: 25, y: 35 }, 2);
    expectTransform(image, { x: 15, y: 15, scale: 1.64 });
  });

  it("放大后可拖动，并限制到 75% 可见边界", async () => {
    const { dialog, image } = await renderLoadedViewer({
      name: "large.png",
      natural: { width: 800, height: 600 },
    });

    await fireEvent.wheel(dialog, { deltaY: -400 });
    await dragImage(image, { x: 0, y: 0 }, { x: 2000, y: 2000 });
    expectTransform(image, { x: 456, y: 342, scale: 1.64 });

    await fireEvent.pointerUp(image, { pointerId: 1 });
    await dragImage(image, { x: 0, y: 0 }, { x: -2000, y: -2000 }, 2);
    expectTransform(image, { x: -456, y: -342, scale: 1.64 });
  });

  it("轴向尺寸不足显示区域 75% 时该轴不允许拖偏", async () => {
    const { dialog, image } = await renderLoadedViewer({
      name: "thin.png",
      natural: { width: 200, height: 600 },
    });

    await fireEvent.wheel(dialog, { deltaY: -400 });
    await dragImage(image, { x: 0, y: 0 }, { x: 2000, y: 2000 });

    expectTransform(image, { x: 0, y: 342, scale: 1.64 });
  });

  it("缩小后重新限制偏移，缩回原始倍率后归零", async () => {
    const { dialog, image } = await renderLoadedViewer({
      name: "large.png",
      natural: { width: 800, height: 600 },
    });

    await fireEvent.wheel(dialog, { deltaY: -1000 });
    await dragImage(image, { x: 0, y: 0 }, { x: 2000, y: 2000 });
    expectTransform(image, { x: 840, y: 630, scale: 2.6 });

    await fireEvent.wheel(dialog, { deltaY: 250 });
    expectTransform(image, { x: 424, y: 318, scale: 1.56 });

    await fireEvent.wheel(dialog, { deltaY: 500 });
    expectTransform(image, { x: 0, y: 0, scale: 1 });
  });

  it("切换图片后重置缩放和拖动偏移", async () => {
    const { view, dialog, image } = await renderLoadedViewer({
      name: "large.png",
      natural: { width: 1200, height: 1800 },
    });

    await fireEvent.wheel(dialog, { deltaY: -400 });
    await dragImage(image, { x: 10, y: 20 }, { x: 25, y: 35 });
    expectTransform(image, { x: 15, y: 15 });

    await view.rerender({
      image: {
        src: "asset://next.png",
        name: "next.png",
      },
    });

    const nextImage = view.getByRole("img", { name: "next.png" }) as HTMLImageElement;
    expect(nextImage.style.width).toBe("");
    expect(nextImage.style.height).toBe("");
    expectTransform(nextImage, { x: 0, y: 0, scale: 1 });
    expect(nextImage.style.cursor).toBe("default");
  });
});
