import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import ImageViewer from "../src/components/chat/ImageViewer.vue";

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
    Object.defineProperty(image, "naturalWidth", { configurable: true, value: 640 });
    Object.defineProperty(image, "naturalHeight", { configurable: true, value: 480 });

    await fireEvent.load(image);

    expect(view.getByText("图片 1.png")).toBeInTheDocument();
    expect(view.getByText("640 x 480 · PNG · 1.5 KB")).toBeInTheDocument();
  });

  it("滚轮缩放后可用左键拖动图片", async () => {
    const view = render(ImageViewer, {
      props: {
        image: {
          src: "asset://shot.png",
          name: "shot.png",
        },
      },
    });
    const dialog = view.getByRole("dialog", { name: "图片查看器" });
    const image = view.getByRole("img", { name: "shot.png" }) as HTMLImageElement;
    image.setPointerCapture = () => {};
    image.releasePointerCapture = () => {};

    await fireEvent.wheel(dialog, { deltaY: -400 });
    expect(image.style.transform).toContain("scale(1.64");

    await fireEvent.pointerDown(image, {
      button: 0,
      pointerId: 1,
      clientX: 10,
      clientY: 20,
    });
    await fireEvent.pointerMove(image, {
      pointerId: 1,
      clientX: 25,
      clientY: 35,
    });

    expect(image.style.transform).toContain("translate3d(15px, 15px, 0)");

    await fireEvent.pointerUp(image, { pointerId: 1 });
  });
});
