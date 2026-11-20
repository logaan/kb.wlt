#include "lvgl.h"
#include "src/core/lv_refr_private.h"
#include "src/display/lv_display_private.h"
#include "src/misc/lv_area_private.h"

static lv_layer_t layer;
static lv_draw_buf_t buffer;
static lv_display_t drawing_display;

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
    drawing_display.hor_res = width;
    drawing_display.ver_res = top + rows;
    drawing_display.antialiasing = true;
    drawing_display.color_format = LV_COLOR_FORMAT_RGB565;
}

static void draw_rectangle(const lv_area_t *area, lv_color_t color, int32_t radius, bool outline)
{
    lv_area_t intersection;
    if (!lv_area_intersect(&intersection, &layer._clip_area, area)) {
        return;
    }

    lv_draw_rect_dsc_t rectangle;
    lv_draw_rect_dsc_init(&rectangle);
    rectangle.radius = radius;
    if (outline) {
        rectangle.bg_opa = LV_OPA_TRANSP;
        rectangle.border_color = color;
        rectangle.border_width = 1;
    }
    else {
        rectangle.bg_color = color;
    }
    lv_draw_rect(&layer, &rectangle, area);
    dispatch();
}

void kb_lvgl_rect(uint16_t x, uint16_t y, uint16_t width, uint16_t height,
                  uint32_t rgb, uint8_t radius, uint8_t outline)
{
    lv_area_t area = {x, y, x + width - 1, y + height - 1};
    draw_rectangle(&area, lv_color_hex(rgb), radius, outline);
}

void kb_lvgl_line(uint16_t x1, uint16_t y1, uint16_t x2, uint16_t y2,
                  uint32_t rgb, uint8_t width)
{
    lv_area_t area = {LV_MIN(x1, x2) - width, LV_MIN(y1, y2) - width,
                      LV_MAX(x1, x2) + width, LV_MAX(y1, y2) + width};
    lv_area_t intersection;
    if (!lv_area_intersect(&intersection, &layer._clip_area, &area)) {
        return;
    }

    lv_draw_line_dsc_t line;
    lv_draw_line_dsc_init(&line);
    line.p1.x = x1;
    line.p1.y = y1;
    line.p2.x = x2;
    line.p2.y = y2;
    line.color = lv_color_hex(rgb);
    line.width = width;
    line.round_start = true;
    line.round_end = true;
    lv_refr_set_disp_refreshing(&drawing_display);
    lv_draw_line(&layer, &line);
    dispatch();
    lv_refr_set_disp_refreshing(NULL);
}

static int32_t rounded_trig(int32_t position)
{
    int32_t half = 1 << (LV_TRIGO_SHIFT - 1);
    return (position + half) >> LV_TRIGO_SHIFT;
}

static void draw_arc_cap(const lv_draw_arc_dsc_t *arc, int32_t angle)
{
    int32_t radius = arc->radius - arc->width / 2;
    int32_t x = arc->center.x + rounded_trig(radius * lv_trigo_cos(angle)) - arc->width / 2;
    int32_t y = arc->center.y + rounded_trig(radius * lv_trigo_sin(angle)) - arc->width / 2;
    lv_area_t area = {x, y, x + arc->width - 1, y + arc->width - 1};
    draw_rectangle(&area, arc->color, LV_RADIUS_CIRCLE, false);
}

void kb_lvgl_arc(uint16_t x, uint16_t y, uint16_t radius, uint16_t start,
                 uint8_t sweep, uint32_t rgb, uint8_t width)
{
    lv_area_t area = {x - radius, y - radius, x + radius, y + radius};
    lv_area_t intersection;
    if (!lv_area_intersect(&intersection, &layer._clip_area, &area)) {
        return;
    }

    lv_draw_arc_dsc_t arc;
    lv_draw_arc_dsc_init(&arc);
    arc.center.x = x;
    arc.center.y = y;
    arc.radius = radius;
    arc.width = width;
    arc.start_angle = start;
    arc.end_angle = start + sweep;
    arc.color = lv_color_hex(rgb);
    lv_draw_arc(&layer, &arc);
    dispatch();
    draw_arc_cap(&arc, start);
    draw_arc_cap(&arc, start + sweep);
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
