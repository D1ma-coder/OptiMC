use super::*;
impl Ui {
    pub(super) fn option_slider(
        &mut self,
        label: &str,
        rect: Rect,
        value: f32,
        kind: Drag,
    ) -> Option<f32> {
        if self.click == Some(MouseButton::Left) && rect.contains(self.mouse) {
            self.drag = Some(kind);
        }
        let next = if self.held && self.drag == Some(kind) {
            Some(((self.mouse[0] - rect.x - 4.) / (rect.w - 8.)).clamp(0., 1.))
        } else {
            None
        };
        self.button("", rect, true);
        self.fill(
            Rect {
                x: rect.x + value.clamp(0., 1.) * (rect.w - 8.),
                w: 8.,
                ..rect
            },
            [0.7, 0.7, 0.7, 1.],
        );
        self.text(
            label,
            (rect.x + (rect.w - self.text_width(label)) * 0.5).round(),
            rect.y + 6.,
            [1.; 4],
        );
        next
    }
    pub(super) fn options_screen(&mut self, settings: &mut Settings, actions: &mut Vec<Action>) {
        let c = (self.width * 0.5).floor();
        let h = self.height;
        let rect = |x, y, w| Rect { x, y, w, h: 20. };
        match self.screen {
            Screen::Options => {
                self.centered("Настройки", 16., [1.; 4]);
                let label = match settings.fov {
                    70 => "Поле зрения: Нормально".into(),
                    110 => "Поле зрения: Quake Pro".into(),
                    n => format!("Поле зрения: {n}"),
                };
                if let Some(value) = self.option_slider(
                    &label,
                    rect(c - 155., 40., 310.),
                    (settings.fov - 30) as f32 / 80.,
                    Drag::Fov,
                ) {
                    let fov = (30. + value * 80.).round() as u32;
                    if fov != settings.fov {
                        settings.fov = fov;
                        actions.push(Action::Changed);
                    }
                }
                if self.button("Настройки графики...", rect(c - 155., 76., 150.), true)
                {
                    self.screen = Screen::Graphics;
                }
                if self.button("Настройки мыши...", rect(c + 5., 76., 150.), true) {
                    self.screen = Screen::MouseSettings;
                }
                if self.button("Настройки OptiMC...", rect(c - 155., 100., 310.), true) {
                    self.screen = Screen::OptiMC;
                }
                if self.button("Приближение (Zoomify)...", rect(c - 155., 126., 310.), true)
                {
                    self.screen = Screen::ZoomSettings;
                }
                if self.button("Объёмные слои скина...", rect(c - 155., 152., 310.), true)
                {
                    self.screen = Screen::SkinLayerSettings;
                }
                if self.button("Готово", rect(c - 100., h - 28., 200.), true) {
                    self.screen = if self.in_world {
                        Screen::Paused
                    } else {
                        Screen::Main
                    };
                }
            }
            Screen::SkinLayerSettings => {
                self.centered("Объёмные слои скина", 12., [1.; 4]);
                let c = self.width * 0.5;
                let config = &mut settings.skin_layers;
                if self.button(
                    &format!("3D-слои: {}", if config.enabled { "Вкл" } else { "Выкл" }),
                    rect(c - 155., 32., 310.),
                    true,
                ) {
                    config.enabled = !config.enabled;
                    actions.push(Action::Changed);
                }
                for (i, (label, value)) in [
                    ("Шапка", &mut config.enable_hat),
                    ("Куртка", &mut config.enable_jacket),
                    ("Левый рукав", &mut config.enable_left_sleeve),
                    ("Правый рукав", &mut config.enable_right_sleeve),
                    ("Левая штанина", &mut config.enable_left_pants),
                    ("Правая штанина", &mut config.enable_right_pants),
                ]
                .into_iter()
                .enumerate()
                {
                    if self.button(
                        &format!("{label}: {}", if *value { "3D" } else { "Плоский" }),
                        rect(
                            c - 155. + (i % 2) as f32 * 160.,
                            58. + (i / 2) as f32 * 26.,
                            150.,
                        ),
                        true,
                    ) {
                        *value = !*value;
                        actions.push(Action::Changed);
                    }
                }
                let label = format!("Размер вокселей: {:.2}", config.base_voxel_size);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c - 155., 136., 150.),
                    config.base_voxel_size - 1.,
                    Drag::SkinVoxelSize,
                ) {
                    config.base_voxel_size = 1. + v;
                    actions.push(Action::Changed);
                }
                let label = format!("Размер шапки: {:.2}", config.head_voxel_size);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c + 5., 136., 150.),
                    config.head_voxel_size - 1.,
                    Drag::SkinHeadSize,
                ) {
                    config.head_voxel_size = 1. + v;
                    actions.push(Action::Changed);
                }
                if self.button(
                    &format!(
                        "Fast render: {}",
                        if config.fast_render {
                            "Вкл"
                        } else {
                            "Выкл"
                        }
                    ),
                    rect(c - 155., 162., 150.),
                    true,
                ) {
                    config.fast_render = !config.fast_render;
                    actions.push(Action::Changed);
                }
                let label = format!("Дальность 3D: {:.0}", config.render_distance_lod);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c + 5., 162., 150.),
                    (config.render_distance_lod - 1.) / 31.,
                    Drag::SkinDistance,
                ) {
                    config.render_distance_lod = 1. + (v * 31.).round();
                    actions.push(Action::Changed);
                }
                let label = format!("Ширина куртки: {:.2}", config.body_voxel_width_size);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c - 155., 188., 150.),
                    config.body_voxel_width_size - 1.,
                    Drag::SkinBodySize,
                ) {
                    config.body_voxel_width_size = 1. + v;
                    actions.push(Action::Changed);
                }
                let label = format!(
                    "Рука от 1-го лица: {:.2}",
                    config.first_person_pixel_scaling
                );
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c + 5., 188., 150.),
                    config.first_person_pixel_scaling - 1.,
                    Drag::SkinFirstPersonSize,
                ) {
                    config.first_person_pixel_scaling = 1. + v;
                    actions.push(Action::Changed);
                }
                if self.button("Готово", rect(c - 100., h - 28., 200.), true) {
                    self.screen = Screen::Options;
                }
            }
            Screen::ZoomSettings => {
                use crate::mods::zoom::KeyBehavior;
                self.centered("Приближение — C + колесо", 12., [1.; 4]);
                let z = &mut settings.zoom;
                if self.button(
                    &format!("Зум: {}", if z.enabled { "Вкл" } else { "Выкл" }),
                    rect(c - 155., 32., 150.),
                    true,
                ) {
                    z.enabled = !z.enabled;
                    actions.push(Action::Changed);
                }
                if self.button(
                    if z.zoom_key_behaviour == KeyBehavior::Hold {
                        "Клавиша: удержание"
                    } else {
                        "Клавиша: переключение"
                    },
                    rect(c + 5., 32., 150.),
                    true,
                ) {
                    z.zoom_key_behaviour = if z.zoom_key_behaviour == KeyBehavior::Hold {
                        KeyBehavior::Toggle
                    } else {
                        KeyBehavior::Hold
                    };
                    actions.push(Action::Changed);
                }
                let label = format!("Увеличение: {}x", z.initial_zoom);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c - 155., 58., 310.),
                    (z.initial_zoom.saturating_sub(1)) as f32 / 15.,
                    Drag::ZoomAmount,
                ) {
                    z.initial_zoom = 1 + (v * 15.).round() as u32;
                    actions.push(Action::Changed);
                }
                let label = format!("Приближение: {:.2}с", z.zoom_in_time);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c - 155., 84., 150.),
                    z.zoom_in_time as f32 / 2.,
                    Drag::ZoomInTime,
                ) {
                    z.zoom_in_time = v as f64 * 2.;
                    actions.push(Action::Changed);
                }
                let label = format!("Отдаление: {:.2}с", z.zoom_out_time);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c + 5., 84., 150.),
                    z.zoom_out_time as f32 / 2.,
                    Drag::ZoomOutTime,
                ) {
                    z.zoom_out_time = v as f64 * 2.;
                    actions.push(Action::Changed);
                }
                if self.button(
                    &format!(
                        "Зум колесом: {}",
                        if z.scroll_zoom { "Вкл" } else { "Выкл" }
                    ),
                    rect(c - 155., 110., 150.),
                    true,
                ) {
                    z.scroll_zoom = !z.scroll_zoom;
                    actions.push(Action::Changed);
                }
                if self.button(
                    &format!(
                        "Помнить шаг: {}",
                        if z.retain_zoom_steps {
                            "Вкл"
                        } else {
                            "Выкл"
                        }
                    ),
                    rect(c + 5., 110., 150.),
                    true,
                ) {
                    z.retain_zoom_steps = !z.retain_zoom_steps;
                    actions.push(Action::Changed);
                }
                let label = format!("Ступеней: {}", z.scroll_step_count);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c - 155., 136., 150.),
                    z.scroll_step_count as f32 / 30.,
                    Drag::ZoomSteps,
                ) {
                    z.scroll_step_count = (v * 30.).round() as i32;
                    actions.push(Action::Changed);
                }
                let label = format!("Шаг зума: {}%", z.zoom_per_step);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c + 5., 136., 150.),
                    (z.zoom_per_step - 100) as f32 / 200.,
                    Drag::ZoomStepAmount,
                ) {
                    z.zoom_per_step = 100 + (v * 200.).round() as u32;
                    actions.push(Action::Changed);
                }
                let label = format!("Прокрутка: {}%", z.scroll_zoom_smoothness);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c - 155., 162., 150.),
                    z.scroll_zoom_smoothness as f32 / 100.,
                    Drag::ZoomSmoothness,
                ) {
                    z.scroll_zoom_smoothness = (v * 100.).round().max(1.) as u32;
                    actions.push(Action::Changed);
                }
                let label = format!("Чувств. зума: {}%", z.relative_sensitivity);
                if let Some(v) = self.option_slider(
                    &label,
                    rect(c + 5., 162., 150.),
                    z.relative_sensitivity as f32 / 100.,
                    Drag::ZoomSensitivity,
                ) {
                    z.relative_sensitivity = (v * 100.).round() as u32;
                    actions.push(Action::Changed);
                }
                if self.button("Готово", rect(c - 100., h - 28., 200.), true) {
                    self.screen = Screen::Options;
                }
            }
            Screen::MouseSettings => {
                self.centered("Настройки мыши", 16., [1.; 4]);
                let label = format!("Чувствительность: {:.0}%", settings.sensitivity * 200.);
                if let Some(value) = self.option_slider(
                    &label,
                    rect(c - 155., 40., 310.),
                    settings.sensitivity as f32,
                    Drag::Sensitivity,
                ) {
                    settings.sensitivity = value as f64;
                    actions.push(Action::Changed);
                }
                for (i, (axis, value)) in [
                    ("X", &mut settings.invert_x_mouse),
                    ("Y", &mut settings.invert_y_mouse),
                ]
                .into_iter()
                .enumerate()
                {
                    if self.button(
                        &format!("Инверсия {axis}: {}", if *value { "Вкл" } else { "Выкл" }),
                        rect(c - 155. + i as f32 * 160., 66., 150.),
                        true,
                    ) {
                        *value = !*value;
                        actions.push(Action::Changed);
                    }
                }
                let fraction =
                    (settings.wheel_sensitivity.ln() - 0.01f64.ln()) / (10f64.ln() - 0.01f64.ln());
                let label = format!("Чувств. прокрутки: {:.2}", settings.wheel_sensitivity);
                if let Some(value) = self.option_slider(
                    &label,
                    rect(c - 155., 92., 310.),
                    fraction as f32,
                    Drag::WheelSensitivity,
                ) {
                    settings.wheel_sensitivity =
                        (0.01f64.ln() + value as f64 * (10f64.ln() - 0.01f64.ln())).exp();
                    actions.push(Action::Changed);
                }
                if self.button(
                    &format!(
                        "Дискретная прокрутка: {}",
                        if settings.discrete_scroll {
                            "Вкл"
                        } else {
                            "Выкл"
                        }
                    ),
                    rect(c - 155., 118., 310.),
                    true,
                ) {
                    settings.discrete_scroll = !settings.discrete_scroll;
                    actions.push(Action::Changed);
                }
                if self.button("Готово", rect(c - 100., h - 28., 200.), true) {
                    self.screen = Screen::Options;
                }
            }
            _ => {}
        }
    }
}
