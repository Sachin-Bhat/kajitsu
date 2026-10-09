use amane::{Canvas, Cap, Color, Path, Shape, shapes};

const FRAME: &[(f32, f32, f32, f32)] = &[
    (1.5, 1.5, 16.5, 1.5),
    (16.5, 1.5, 16.5, 16.5),
    (16.5, 16.5, 1.5, 16.5),
    (1.5, 16.5, 1.5, 1.5),
];

pub fn view(name: Option<&str>, color: Color) -> Canvas {
    let name = name.unwrap_or_default();
    let (name, vertical) = name
        .strip_prefix("vertical_")
        .map_or((name, false), |name| (name, true));
    let divisions: &[(f32, f32, f32, f32)] = match name {
        "tile" => &[(10.0, 1.5, 10.0, 16.5), (10.0, 9.0, 16.5, 9.0)],
        "right_tile" => &[(8.0, 1.5, 8.0, 16.5), (1.5, 9.0, 8.0, 9.0)],
        "center_tile" => &[
            (5.0, 1.5, 5.0, 16.5),
            (13.0, 1.5, 13.0, 16.5),
            (1.5, 9.0, 5.0, 9.0),
            (13.0, 9.0, 16.5, 9.0),
        ],
        "grid" => &[
            (6.5, 1.5, 6.5, 16.5),
            (11.5, 1.5, 11.5, 16.5),
            (1.5, 9.0, 16.5, 9.0),
        ],
        "fair" => &[
            (6.5, 1.5, 6.5, 16.5),
            (11.5, 1.5, 11.5, 16.5),
            (6.5, 9.0, 16.5, 9.0),
        ],
        "dwindle" => &[
            (9.0, 1.5, 9.0, 16.5),
            (9.0, 9.0, 16.5, 9.0),
            (13.0, 9.0, 13.0, 16.5),
        ],
        "deck" => &[
            (10.0, 1.5, 10.0, 16.5),
            (10.0, 4.5, 16.5, 4.5),
            (10.0, 7.0, 16.5, 7.0),
        ],
        "monocle" => &[],
        "scroller" => &[
            (1.5, 4.0, 4.5, 4.0),
            (4.5, 4.0, 4.5, 14.0),
            (4.5, 14.0, 1.5, 14.0),
            (6.5, 1.5, 11.5, 1.5),
            (11.5, 1.5, 11.5, 16.5),
            (11.5, 16.5, 6.5, 16.5),
            (6.5, 16.5, 6.5, 1.5),
            (16.5, 4.0, 13.5, 4.0),
            (13.5, 4.0, 13.5, 14.0),
            (13.5, 14.0, 16.5, 14.0),
        ],
        _ => &[
            (4.0, 9.0, 5.0, 9.0),
            (8.5, 9.0, 9.5, 9.0),
            (13.0, 9.0, 14.0, 9.0),
        ],
    };
    let frame = if name == "scroller" { &[][..] } else { FRAME };
    let mut path = Path::new();
    for &(x1, y1, x2, y2) in frame.iter().chain(divisions) {
        let (x1, y1, x2, y2) = if vertical {
            (y1, x1, y2, x2)
        } else {
            (x1, y1, x2, y2)
        };
        path = path.move_to(x1, y1).line_to(x2, y2);
    }
    Canvas::new()
        .width(18.0)
        .height(18.0)
        .shapes(shapes![path.stroke(1.25, color).cap(Cap::Round)])
}
