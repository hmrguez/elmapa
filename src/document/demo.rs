use super::{AtlasStyle, Brush, DOCUMENT_VERSION, Document, Layer, Point, Stroke};

impl Document {
    /// The editable example shown on new atlas shelves.
    pub fn spain_example() -> Result<Self, String> {
        let mut document: Self =
            serde_json::from_slice(include_bytes!("../../assets/atlases/espana.elmapa"))
                .map_err(|error| format!("Could not open the Spain example: {error}"))?;
        document.style = AtlasStyle::Woodland;
        document.validate()?;
        Ok(document)
    }

    /// Small deterministic fixture used by the demo export and verification.
    pub fn demo() -> Self {
        fn stroke(brush: Brush, size: f32, seed: u64, points: &[(f32, f32)]) -> Stroke {
            Stroke {
                creation_id: None,
                material: None,
                brush,
                size,
                seed,
                strength: 1.0,
                points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            }
        }
        fn layer(id: u64, name: &str, strokes: Vec<Stroke>) -> Layer {
            Layer {
                id,
                name: name.into(),
                visible: true,
                opacity: 1.0,
                strokes: strokes.into(),
                labels: Vec::new(),
                landmarks: Vec::new(),
                terrain_replay: None,
                territories: None,
            }
        }
        Self {
            version: DOCUMENT_VERSION,
            style: AtlasStyle::Woodland,
            name: "The Stillwater Isles".into(),
            width: 1600.0,
            height: 1000.0,
            next_id: 5,
            layers: vec![
                layer(
                    1,
                    "Land & shoreline",
                    vec![
                        stroke(
                            Brush::Land,
                            290.0,
                            17,
                            &[
                                (430.0, 285.0),
                                (525.0, 250.0),
                                (625.0, 260.0),
                                (710.0, 330.0),
                                (800.0, 380.0),
                                (900.0, 420.0),
                                (1040.0, 440.0),
                                (1140.0, 530.0),
                                (1110.0, 630.0),
                                (990.0, 660.0),
                                (880.0, 650.0),
                                (810.0, 740.0),
                            ],
                        ),
                        stroke(
                            Brush::Land,
                            260.0,
                            29,
                            &[
                                (490.0, 360.0),
                                (525.0, 450.0),
                                (600.0, 515.0),
                                (710.0, 520.0),
                                (805.0, 565.0),
                                (900.0, 600.0),
                            ],
                        ),
                        stroke(
                            Brush::Land,
                            145.0,
                            43,
                            &[(395.0, 650.0), (365.0, 710.0), (390.0, 760.0)],
                        ),
                        stroke(Brush::Land, 86.0, 71, &[(1210.0, 295.0), (1240.0, 335.0)]),
                        stroke(Brush::Land, 52.0, 79, &[(1160.0, 245.0)]),
                    ],
                ),
                layer(
                    3,
                    "Highlands",
                    vec![
                        stroke(
                            Brush::Mountain,
                            125.0,
                            200,
                            &[
                                (585.0, 285.0),
                                (650.0, 320.0),
                                (705.0, 370.0),
                                (755.0, 420.0),
                                (790.0, 475.0),
                            ],
                        ),
                        stroke(
                            Brush::Mountain,
                            80.0,
                            225,
                            &[(975.0, 600.0), (950.0, 640.0), (915.0, 665.0)],
                        ),
                    ],
                ),
                layer(
                    2,
                    "Woodlands",
                    vec![
                        stroke(
                            Brush::Forest,
                            120.0,
                            100,
                            &[
                                (460.0, 310.0),
                                (490.0, 365.0),
                                (560.0, 400.0),
                                (620.0, 455.0),
                            ],
                        ),
                        stroke(
                            Brush::Forest,
                            150.0,
                            125,
                            &[
                                (855.0, 470.0),
                                (930.0, 500.0),
                                (1000.0, 545.0),
                                (1040.0, 605.0),
                            ],
                        ),
                        stroke(Brush::Forest, 85.0, 140, &[(800.0, 645.0), (820.0, 700.0)]),
                        stroke(Brush::Forest, 65.0, 159, &[(380.0, 700.0), (390.0, 740.0)]),
                    ],
                ),
                layer(
                    4,
                    "Rivers & trails",
                    vec![
                        stroke(
                            Brush::River,
                            8.0,
                            300,
                            &[
                                (730.0, 410.0),
                                (710.0, 445.0),
                                (726.0, 485.0),
                                (705.0, 520.0),
                                (735.0, 560.0),
                                (730.0, 605.0),
                                (755.0, 650.0),
                            ],
                        ),
                        stroke(
                            Brush::River,
                            5.0,
                            310,
                            &[
                                (790.0, 475.0),
                                (765.0, 505.0),
                                (735.0, 520.0),
                                (720.0, 545.0),
                            ],
                        ),
                        stroke(
                            Brush::Path,
                            4.0,
                            400,
                            &[
                                (510.0, 460.0),
                                (575.0, 485.0),
                                (640.0, 490.0),
                                (710.0, 520.0),
                                (795.0, 545.0),
                                (870.0, 595.0),
                                (930.0, 600.0),
                            ],
                        ),
                    ],
                ),
            ]
            .into(),
        }
    }
}
