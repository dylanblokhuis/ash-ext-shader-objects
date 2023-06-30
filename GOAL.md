- get a cornell box and egui on the screen
- learn restir http://www.zyanidelab.com/restir-el-cheapo/
  - implement RIS (1 sample per pixel)
  -  Think about what happens with RIS, in one frame it might find an awesome light and maybe the next frame it selects a not so good one and maybe the next frame it selects a terrible light, all of that brings in noise. Could we somehow reuse those samples from previous frames to improve the overall quality? Yes, we can use Reservoir-based Spatio-Temporal Importance Resampling (ReSTIR)
  