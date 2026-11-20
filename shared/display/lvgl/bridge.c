#include "lvgl.h"
#include "src/misc/lv_area_private.h"

static lv_layer_t layer;
static lv_draw_buf_t buffer;

static void dispatch(void)
{
    while (layer.draw_task_head != NULL) {
        lv_draw_dispatch_layer(NULL, &layer);
    }
}

void kb_lvgl_begin(uint16_t *pixels, uint16_t width, uint16_t top, uint16_t rows, uint32_t rgb)
{
    if (!lv_is_initialized()) {
        lv_init();
    }

    uint16_t background = lv_color_to_u16(lv_color_hex(rgb));
    for (uint32_t index = 0; index < (uint32_t)width * rows; index++) {
        pixels[index] = background;
    }

    lv_draw_buf_init(&buffer, width, rows, LV_COLOR_FORMAT_RGB565, width * 2,
                     pixels, width * rows * 2);
    lv_area_t area = {0, top, width - 1, top + rows - 1};
    lv_memzero(&layer, sizeof(layer));
    layer.draw_buf = &buffer;
    layer.buf_area = area;
    layer._clip_area = area;
    layer.phy_clip_area = area;
    layer.color_format = LV_COLOR_FORMAT_RGB565;
}

void kb_lvgl_rect(uint16_t x, uint16_t y, uint16_t width, uint16_t height,
                  uint32_t rgb, uint8_t radius, uint8_t outline)
{
    lv_area_t area = {x, y, x + width - 1, y + height - 1};
    lv_area_t intersection;
    if (!lv_area_intersect(&intersection, &layer._clip_area, &area)) {
        return;
    }

    lv_draw_rect_dsc_t rectangle;
    lv_draw_rect_dsc_init(&rectangle);
    rectangle.radius = radius;
    if (outline) {
        rectangle.bg_opa = LV_OPA_TRANSP;
        rectangle.border_color = lv_color_hex(rgb);
        rectangle.border_width = 1;
    }
    else {
        rectangle.bg_color = lv_color_hex(rgb);
    }
    lv_draw_rect(&layer, &rectangle, &area);
    dispatch();
}

void kb_lvgl_text(uint16_t x, uint16_t y, uint32_t rgb, uint8_t size, const char *text)
{
    const lv_font_t *font = LV_FONT_DEFAULT;
    switch (size) {
#ifdef KB_LVGL_MIP
    case 2: font = &lv_font_montserrat_32; break;
#endif
#ifdef KB_LVGL_TFT
    case 3: font = &lv_font_montserrat_28; break;
    case 4: font = &lv_font_montserrat_36; break;
    case 5: font = &lv_font_montserrat_48; break;
#endif
    default: break;
    }
    lv_area_t area = {x, y, layer.buf_area.x2, y + font->line_height - 1};
    lv_area_t intersection;
    if (!lv_area_intersect(&intersection, &layer._clip_area, &area)) {
        return;
    }

    lv_draw_label_dsc_t label;
    lv_draw_label_dsc_init(&label);
    label.color = lv_color_hex(rgb);
    label.text = text;
    label.font = font;
    label.flag = LV_TEXT_FLAG_EXPAND;
    lv_draw_label(&layer, &label, &area);
    dispatch();
}
