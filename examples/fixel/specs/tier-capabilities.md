# Fixel Density Tier Capabilities

This document details the computational capabilities, typical applications, and limitations for each Fixel density tier.

---

## NANO Tier - IoT and Sensors

**Target Resolution**: 64x64 to 128x128 pixels
**Transistors/Pixel**: 100
**SRAM/Pixel**: 16 bytes
**Power/Pixel**: 0.01 uW

### Compute Capabilities

| Capability | Description | Performance |
|------------|-------------|-------------|
| Threshold Detection | Binary pixel classification | Real-time at 10 MHz |
| Simple Filters | 3x3 Sobel, Prewitt, box blur | 1 filter/frame |
| Morphological Ops | Erosion, dilation (binary) | Single-pass |
| Pixel Comparison | Min, max, difference | Per-cycle |
| Noise Reduction | Simple averaging | 3x3 kernel max |

### Typical Applications

- **Motion Detection Sensors**: Detect movement for security or automation
- **Light Level Monitoring**: Ambient light sensing for display adjustment
- **Presence Detection**: Simple occupancy sensing for smart buildings
- **Edge Triggering**: Wake-on-motion for low-power systems
- **Environmental Sensors**: Basic visual inspection for IoT deployments

### Limitations

- No floating-point arithmetic (integer only)
- Maximum 3x3 convolution kernels
- No neural network support
- Single-bit per-pixel output for most operations
- Minimal temporal memory (1-2 previous frames)
- No color processing (grayscale only in most configurations)

### Example Use Cases

```
Smart Home:     Motion-activated lighting trigger
Agriculture:    Basic crop monitoring (presence/absence)
Industrial:     Simple defect detection (binary classification)
Security:       Ultra-low-power intrusion detection
```

---

## MICRO Tier - Wearables

**Target Resolution**: 256x256 to 480x480 pixels
**Transistors/Pixel**: 10,000
**SRAM/Pixel**: 64 bytes
**Power/Pixel**: 0.1 uW

### Compute Capabilities

| Capability | Description | Performance |
|------------|-------------|-------------|
| Edge Detection | Canny, Sobel, Laplacian | Real-time |
| Basic Classification | <1M parameter networks | 10-30 fps |
| Gesture Recognition | Hand gesture detection | Real-time |
| Face Detection | Haar cascades, simple CNN | 15-30 fps |
| Histogram Processing | Local histogram equalization | Per-frame |
| Basic Optical Flow | Lucas-Kanade style | Sparse features |

### Typical Applications

- **Smartwatch Face Unlock**: Low-power biometric authentication
- **AR Glasses Eye Tracking**: Gaze detection for interaction
- **Fitness Tracker Motion**: Activity recognition and counting
- **Health Monitor Vitals**: Heart rate from camera, SpO2 estimation
- **Gesture Interfaces**: Hand tracking for wearable control

### Limitations

- Neural networks limited to ~1M parameters
- No real-time full-resolution video processing
- INT8 quantization only (no FP16/FP32)
- Convolution kernels up to 5x5
- Limited batch processing (single image)
- Color processing limited to simple transformations

### Example Use Cases

```
Wearables:      Continuous heart rate monitoring via PPG camera
AR/VR:          Eye tracking for foveated rendering hints
Health:         Sleep quality detection from movement patterns
Accessibility:  Simple sign language gesture recognition
```

---

## STANDARD Tier - Consumer Electronics

**Target Resolution**: 1920x1080 to 2560x1440 pixels
**Transistors/Pixel**: 500,000
**SRAM/Pixel**: 256 bytes
**Power/Pixel**: 0.3 uW

### Compute Capabilities

| Capability | Description | Performance |
|------------|-------------|-------------|
| Object Detection | YOLO-style architectures | 30-60 fps |
| Semantic Segmentation | Per-pixel classification | Real-time |
| Pose Estimation | Body/hand keypoint detection | 30 fps |
| Face Recognition | Embedding extraction | Real-time |
| Scene Understanding | Classification + detection | 30 fps |
| Video Stabilization | Motion estimation + correction | Real-time |
| HDR Processing | Tone mapping, local contrast | Per-frame |

### Typical Applications

- **Smartphone Computational Photography**: Real-time enhancement, bokeh, night mode
- **Tablet Augmented Reality**: Object placement, surface detection
- **Laptop Video Conferencing**: Background blur, lighting correction
- **Consumer Drones**: Obstacle avoidance, subject tracking
- **Smart Home Cameras**: Person detection, package detection

### Limitations

- Neural networks limited to ~50M parameters
- No generative model inference
- Specific model architectures required (optimized for in-pixel)
- Power budget constraints for mobile battery life
- Limited transformer support (attention is memory-intensive)

### Example Use Cases

```
Photography:    Real-time portrait mode with hair-level segmentation
Security:       Person vs. pet detection for smart notifications
Automotive:     Driver attention monitoring via face tracking
Retail:         Customer counting and flow analysis
```

---

## PRO Tier - Professional Displays

**Target Resolution**: 3840x2160 pixels (4K)
**Transistors/Pixel**: 2,300,000
**SRAM/Pixel**: 512 bytes
**Power/Pixel**: 0.6 uW

### Compute Capabilities

| Capability | Description | Performance |
|------------|-------------|-------------|
| Neural Inference | Full CNN/RNN architectures | Real-time |
| 4K Video Processing | All operations at 4K | 60 fps |
| Multi-Object Tracking | Dozens of simultaneous objects | Real-time |
| Medical Imaging | CT/MRI enhancement, analysis | Real-time |
| Style Transfer | Artistic filters | 30 fps |
| Depth Estimation | Monocular depth prediction | Real-time |
| Super-Resolution | 2-4x upscaling | Real-time |

### Typical Applications

- **Professional Video Editing**: Real-time color grading, effects preview
- **Medical Diagnostic Imaging**: X-ray analysis, tumor detection assistance
- **Broadcast Enhancement**: Live sports analysis, instant replay enhancement
- **Industrial Inspection**: High-precision defect detection
- **Autonomous Vehicles**: Perception system acceleration
- **Surveillance Systems**: Multi-camera tracking and analysis

### Limitations

- Large generative models (>1B params) require tiling/streaming
- Training not supported (inference only)
- Some large transformer models exceed local memory
- Active cooling may be required at sustained loads

### Example Use Cases

```
Medical:        Real-time CT scan enhancement during procedures
Broadcast:      Player tracking and statistics overlay for sports
Industrial:     Sub-millimeter defect detection in manufacturing
Automotive:     Full surround-view perception with semantic labels
```

---

## ULTRA Tier - Workstations

**Target Resolution**: 7680x4320 pixels (8K)
**Transistors/Pixel**: 2,300,000
**SRAM/Pixel**: 1024 bytes
**Power/Pixel**: 1.2 uW

### Compute Capabilities

| Capability | Description | Performance |
|------------|-------------|-------------|
| Generative Models | Stable Diffusion, DALL-E style | Near real-time |
| LLM Acceleration | Transformer attention assist | 10-100 tokens/sec |
| Scientific Simulation | Physics, fluid dynamics | Real-time visualization |
| Ray Tracing | Path tracing acceleration | Interactive rates |
| Full Transformers | ViT, BERT inference | Real-time |
| 8K Processing | All operations at 8K | 30-60 fps |
| Multi-Modal AI | Vision + language fusion | Real-time |

### Typical Applications

- **AI Research Workstations**: Model experimentation and visualization
- **Scientific Visualization**: Real-time simulation rendering
- **Film and VFX Production**: AI-assisted rotoscoping, enhancement
- **High-Frequency Trading**: Ultra-low-latency visual analytics
- **Astronomy Research**: Large-scale image analysis
- **Data Center Inference**: Distributed AI acceleration

### Limitations

- High power consumption (40-80W total)
- Premium pricing ($200+)
- Requires active cooling solution
- Inference only (no on-device training)
- Large models may still require multiple passes

### Example Use Cases

```
Research:       Interactive neural network visualization and debugging
Film/VFX:       Real-time AI upscaling and frame interpolation
Science:        Live astronomical image processing and classification
Finance:        Visual pattern recognition for trading signals
```

---

## Tier Comparison Summary

| Tier | Resolution | Trans/Pixel | SRAM/Pixel | Clock | Power/Pixel | Cost |
|------|------------|-------------|------------|-------|-------------|------|
| NANO | 128x128 | 100 | 16 B | 10 MHz | 0.01 uW | $0.50 |
| MICRO | 480x480 | 10K | 64 B | 50 MHz | 0.1 uW | $5 |
| STANDARD | 2560x1440 | 500K | 256 B | 100 MHz | 0.3 uW | $25 |
| PRO | 3840x2160 | 2.3M | 512 B | 500 MHz | 0.6 uW | $80 |
| ULTRA | 7680x4320 | 2.3M | 1024 B | 1 GHz | 1.2 uW | $200 |

---

## Capability Progression

```
NANO    --> Simple filters, thresholds
  |
MICRO   --> Edge detection, basic ML
  |
STANDARD --> Object detection, segmentation
  |
PRO     --> Full neural inference, 4K video
  |
ULTRA   --> Generative AI, scientific simulation
```

Each tier builds upon the capabilities of the previous tier while adding significant new functionality enabled by the increased compute density and memory capacity.
