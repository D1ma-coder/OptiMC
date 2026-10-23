# WindowServer timeout during Vulkan FPS investigation

Graphical testing is suspended after the user reported loss of the desktop/system session. Do not automatically repeat the MoltenVK experiments or launch graphical tests on this machine while investigating this incident. Continue with source inspection, CPU-only tests and saved diagnostics.

Observed on 2026-10-20:

- System reports `WindowServer-2026-10-20-185358.ips` and `WindowServer-2026-10-20-194556.ips` both record WATCHDOG termination after 40 seconds without a successful WindowServer check-in.
- The reports record nominal thermal pressure. They do not establish a kernel panic or the exact cause of the hang.
- The later stackshot contains test process `optimc` PID 20260. Its main thread is blocked through the Vulkan library, AGXMetalG14X, IOGPU, Metal and a semaphore wait. This supports investigating the graphics submission path; it does not prove a particular driver defect.
- `MVK_CONFIG_SYNCHRONOUS_QUEUE_SUBMITS=0` was supplied only to experimental subprocesses. The asynchronous run did not finish its requested frames. Its last timing sample reports acquire 40.117 ms, submit 0.018 ms.
- The ordinary synchronous Immediate run completed at 119.97 FPS with cap 0. Disabling presentation with command buffer also did not remove the 120 FPS ceiling. The FPS issue remains unresolved.
- Experimental MoltenVK environment variables were not added to application source or user settings. No rollback of persisted settings was necessary. The packaged application was not updated with these experiments.
- Process inspection after the incident found no remaining OptiMC processes. No new GPU test was started during incident inspection.

Future reproduction needs isolation and a wall-clock timeout external to the rendering process; a frame-count limit cannot stop a blocked GPU call. A timeout reduces exposure but cannot guarantee recovery from a system graphics hang. Do not launch concurrent GPU experiments on the user's desktop.
