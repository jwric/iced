//! Compute the damage between frames.
use crate::core::{Point, Rectangle};

/// Diffs the damage regions given some previous and current primitives.
///
/// Primitives are paired in order. A single primitive inserted or removed
/// damages only itself: the ones after it are still paired with themselves.
pub fn diff<T>(
    previous: &[T],
    current: &[T],
    bounds: impl Fn(&T) -> Vec<Rectangle>,
    diff: impl Fn(&T, &T) -> Vec<Rectangle>,
) -> Vec<Rectangle> {
    align(previous, current, bounds, &diff, |a, b| {
        diff(a, b).is_empty()
    })
}

/// Computes the damage regions given some previous and current primitives.
///
/// Primitives are paired as in [`diff`].
pub fn list<T>(
    previous: &[T],
    current: &[T],
    bounds: impl Fn(&T) -> Vec<Rectangle>,
    are_equal: impl Fn(&T, &T) -> bool,
) -> Vec<Rectangle> {
    align(
        previous,
        current,
        &bounds,
        |a, b| {
            if are_equal(a, b) {
                vec![]
            } else {
                bounds(a).into_iter().chain(bounds(b)).collect()
            }
        },
        &are_equal,
    )
}

/// Pairs `previous` with `current` in order, looking one primitive ahead
/// when a pair differs: if the next one on either side matches, the
/// primitive before it was inserted or removed.
fn align<T>(
    previous: &[T],
    current: &[T],
    bounds: impl Fn(&T) -> Vec<Rectangle>,
    diff: impl Fn(&T, &T) -> Vec<Rectangle>,
    are_equal: impl Fn(&T, &T) -> bool,
) -> Vec<Rectangle> {
    let mut damage = Vec::new();
    let (mut i, mut j) = (0, 0);

    while let (Some(a), Some(b)) = (previous.get(i), current.get(j)) {
        let changes = diff(a, b);

        if changes.is_empty() {
            i += 1;
            j += 1;
        } else if current.get(j + 1).is_some_and(|next| are_equal(a, next)) {
            damage.extend(bounds(b));
            j += 1;
        } else if previous.get(i + 1).is_some_and(|next| are_equal(next, b)) {
            damage.extend(bounds(a));
            i += 1;
        } else {
            damage.extend(changes);
            i += 1;
            j += 1;
        }
    }

    // Extend damage by the added/removed primitives
    damage.extend(previous[i..].iter().chain(&current[j..]).flat_map(bounds));
    damage
}

/// Groups the given damage regions that are close together inside the given
/// bounds.
pub fn group(mut damage: Vec<Rectangle>, bounds: Rectangle) -> Vec<Rectangle> {
    const AREA_THRESHOLD: f32 = 20_000.0;

    damage.sort_by(|a, b| {
        a.center()
            .distance(Point::ORIGIN)
            .total_cmp(&b.center().distance(Point::ORIGIN))
    });

    let mut output = Vec::new();
    let mut scaled = damage
        .into_iter()
        .filter_map(|region| region.intersection(&bounds))
        .filter(|region| region.width >= 1.0 && region.height >= 1.0);

    if let Some(mut current) = scaled.next() {
        for region in scaled {
            let union = current.union(&region);

            if union.area() - current.area() - region.area() <= AREA_THRESHOLD {
                current = union;
            } else {
                output.push(current);
                current = region;
            }
        }

        output.push(current);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Size;

    /// The damage between two lists of primitives that each cover one point.
    fn damage(previous: &[f32], current: &[f32]) -> Vec<f32> {
        list(
            previous,
            current,
            |x| vec![Rectangle::new(Point::new(*x, 0.0), Size::new(1.0, 1.0))],
            |a, b| a == b,
        )
        .iter()
        .map(|region| region.x)
        .collect()
    }

    #[test]
    fn an_inserted_primitive_damages_only_itself() {
        assert_eq!(damage(&[1.0, 2.0, 3.0], &[1.0, 9.0, 2.0, 3.0]), [9.0]);
    }

    #[test]
    fn a_removed_primitive_damages_only_itself() {
        assert_eq!(damage(&[1.0, 9.0, 2.0, 3.0], &[1.0, 2.0, 3.0]), [9.0]);
    }

    #[test]
    fn a_changed_primitive_damages_where_it_was_and_where_it_is() {
        assert_eq!(damage(&[1.0, 2.0, 3.0], &[1.0, 9.0, 3.0]), [2.0, 9.0]);
    }

    #[test]
    fn primitives_added_or_removed_at_the_end_are_damage() {
        assert_eq!(damage(&[1.0], &[1.0, 2.0, 3.0]), [2.0, 3.0]);
        assert_eq!(damage(&[1.0, 2.0, 3.0], &[1.0]), [2.0, 3.0]);
    }
}
