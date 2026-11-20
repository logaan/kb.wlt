const encoder = new TextEncoder();

export function inlineScene(commands, strings) {
  const result = [];
  for (const { opcode, x, y, w, h, first, second, third } of commands) {
    result.push(opcode, x & 255, x >>> 8, y & 255, y >>> 8);
    if (opcode === 3) {
      const text = strings[third];
      if (text === undefined) throw new Error("Display command references a missing string");
      const bytes = encoder.encode(text);
      if (bytes.length > 255) throw new Error("Display text exceeds the LVGL scene limit");
      result.push(first, second, bytes.length, ...bytes);
    } else if (opcode === 1 || opcode === 2 || opcode === 4) {
      result.push(w & 255, w >>> 8, h & 255, h >>> 8, first, second);
    } else {
      throw new Error(`Unknown display command ${opcode}`);
    }
  }
  if (result.length > 768) throw new Error("Display scene exceeds the LVGL buffer");
  return new Uint8Array(result);
}

export function makeDisplay(canvas, profile, lvgl) {
  const context = canvas.getContext("2d", { alpha: false });
  const { width, height } = canvas;
  const frame = context.createImageData(width, height);
  return (guest, model, detail) => {
    const scene = inlineScene(...guest.render(width, height, model, detail));
    new Uint8Array(lvgl.memory.buffer, lvgl.lvgl_scene_buffer(), scene.length).set(scene);
    for (let y = 0; y < height; y += 8) {
      const rows = Math.min(8, height - y);
      const address = lvgl.lvgl_render(profile, scene.length, y, rows);
      if (!address) throw new Error("LVGL rejected the display scene");
      const strip = new Uint16Array(lvgl.memory.buffer, address, width * rows);
      for (let pixel = 0, offset = y * width * 4; pixel < strip.length; pixel++, offset += 4) {
        const color = strip[pixel];
        const red = (color >> 11) & 31;
        const green = (color >> 5) & 63;
        const blue = color & 31;
        frame.data[offset] = (red << 3) | (red >> 2);
        frame.data[offset + 1] = (green << 2) | (green >> 4);
        frame.data[offset + 2] = (blue << 3) | (blue >> 2);
        frame.data[offset + 3] = 255;
      }
    }
    context.putImageData(frame, 0, 0);
  };
}
