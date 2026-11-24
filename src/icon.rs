//! Иконка приложения, нарисованная программно (без файлов-ресурсов):
//! жёлтый «листок» со скруглёнными углами и тремя строками текста.

pub const SIZE: u32 = 64;

/// Пиксели иконки в формате RGBA, построчно.
pub fn rgba() -> Vec<u8> {
    let s = SIZE as f32;
    let mut out = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let (margin, radius) = (4.0, 10.0);
    let paper = [0xF5, 0xB7, 0x00];
    let line = [0x5A, 0x3E, 0x00];
    // Строки текста: (y, длина в долях ширины).
    let lines = [(22.0, 0.70), (32.0, 0.70), (42.0, 0.45)];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Расстояние до скруглённого прямоугольника (для сглаживания краёв).
            let cx = px.clamp(margin + radius, s - margin - radius);
            let cy = py.clamp(margin + radius, s - margin - radius);
            let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - radius;
            let alpha = (0.5 - dist).clamp(0.0, 1.0);

            let x0 = margin + 10.0;
            let is_line = lines
                .iter()
                .any(|&(ly, len)| (py - ly).abs() <= 2.0 && px >= x0 && px <= x0 + (s - 2.0 * x0) * len / 0.70);
            let c = if is_line { line } else { paper };
            out.extend_from_slice(&[c[0], c[1], c[2], (alpha * 255.0) as u8]);
        }
    }
    out
}

/// Иконка в формате ARGB32 (для трея по протоколу StatusNotifierItem).
#[cfg(target_os = "linux")]
pub fn argb() -> Vec<u8> {
    rgba()
        .chunks_exact(4)
        .flat_map(|p| [p[3], p[0], p[1], p[2]])
        .collect()
}
