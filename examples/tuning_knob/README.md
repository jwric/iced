# Tuning Knob

A custom widget example showcasing an interactive tuning knob that can be rotated by dragging with the mouse or using the scroll wheel.

The `tuning_knob` demonstrates:

- Creating a custom widget using the advanced `Widget` trait
- Handling mouse drag events for interactive controls
- Drawing custom graphics using the renderer's quad primitives
- Managing widget state across frames
- Supporting both mouse drag and scroll wheel input

## Features

- **Drag to adjust**: Click and drag vertically to change the value
- **Scroll wheel support**: Hover and use the mouse wheel for fine adjustments
- **Visual feedback**: Hover state indication and rotating indicator
- **Self-contained widget**: The entire widget implementation is in an inline `knob` module for easy copy-pasting into your own projects

## Usage

The knob widget can be used like any other iced widget:

```rust
use knob::knob;

knob(0.0..=100.0, current_value, Message::ValueChanged)
    .size(100.0)
    .step(0.01)
```

## Running the example

You can run the example with:

```
cargo run -p tuning_knob
```

The example shows three knobs controlling different parameters:
- **Frequency**: 20 Hz to 20,000 Hz
- **Volume**: 0% to 100%
- **Pan**: Left to Right

## Implementation

The widget is built using iced's advanced `Widget` trait, which provides full control over:
- Layout calculation
- Custom rendering
- Event handling
- State management

The knob module is completely self-contained and can be copied directly into your project.