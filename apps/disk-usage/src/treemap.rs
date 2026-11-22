//! Binary subdivision on the longer axis keeps each tile proportional to its byte count.
#[derive(Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
pub fn layout(weights: &[u64]) -> Vec<Rect> {
    fn split(weights: &[u64], r: Rect, out: &mut Vec<Rect>) {
        if weights.len() == 1 {
            out.push(r);
            return;
        }
        let total: f64 = weights.iter().map(|&n| n as f64).sum();
        let mut left = weights[0] as f64;
        let mut i = 1;
        while i + 1 < weights.len()
            && (left + weights[i] as f64 - total / 2.).abs() < (left - total / 2.).abs()
        {
            left += weights[i] as f64;
            i += 1;
        }
        let share = (left / total) as f32;
        let (first, second) = if r.w * 2. > r.h {
            let w = r.w * share;
            (
                Rect { w, ..r },
                Rect {
                    x: r.x + w,
                    w: r.w - w,
                    ..r
                },
            )
        } else {
            let h = r.h * share;
            (
                Rect { h, ..r },
                Rect {
                    y: r.y + h,
                    h: r.h - h,
                    ..r
                },
            )
        };
        split(&weights[..i], first, out);
        split(&weights[i..], second, out);
    }
    let mut tiles = Vec::new();
    if !weights.is_empty() && weights.iter().all(|&n| n > 0) {
        split(
            weights,
            Rect {
                x: 0.,
                y: 0.,
                w: 1.,
                h: 1.,
            },
            &mut tiles,
        );
    }
    tiles
}
#[cfg(test)]
mod tests {
    use super::layout;
    #[test]
    fn each_rectangle_has_its_share_and_stays_inside() {
        for weights in [vec![1], vec![9, 3, 2, 1], vec![u64::MAX, 1, 1]] {
            let total = weights.iter().map(|&w| w as f64).sum::<f64>();
            for (w, r) in weights.iter().zip(layout(&weights)) {
                assert!((f64::from(r.w * r.h) - *w as f64 / total).abs() < 0.000001);
                assert!(r.x + r.w <= 1.000001 && r.y + r.h <= 1.000001);
            }
        }
        assert!(layout(&[]).is_empty());
    }
}
