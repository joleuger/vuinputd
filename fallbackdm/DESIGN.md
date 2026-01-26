# Background

Motivation
When a display server (like X11 or Wayland) takes control of a session, it needs exclusive access to the virtual terminal (VT). Without this, the kernel could still process keyboard input directly at the VT level, causing interference with the display server's input handling. This creates a conflict where both the VT subsystem and display server try to handle the same keystrokes, leading to unpredictable behavior.

Details
The D-Bus TakeControl method initiates this process by validating the caller's permissions and delegating to session_set_controller() [1a]. This function sets up bus tracking to monitor the controller's lifecycle and critically calls session_prepare_vt() [1b] to configure the VT hardware.

The VT preparation performs three essential operations: it changes ownership of the VT device file to the session user, then issues two key ioctl calls. The first, ioctl(vt, KDSKBMODE, K_OFF) [1c], disables keyboard input at the VT level - this is the "muting" that prevents the kernel from processing keystrokes directly. The second, ioctl(vt, KDSETMODE, KD_GRAPHICS) [1d], switches the VT to graphics mode. Finally, it sets VT process mode [1e] to handle VT switches through signals rather than the default kernel mechanism.

This sequence ensures the display server has uninterrupted control over input and output, preventing the kernel from interfering with session management.

Detailed code path:

1
D-Bus TakeControl to VT Muting
Traces the complete flow from D-Bus TakeControl call to VT keyboard muting via ioctl. See guide

method_take_control() entry point
Permission validation

1a
TakeControl calls session_set_controller
logind-session-dbus.c:404
r = session_set_controller(s, sd_bus_message_get_sender(message), force, true);
Bus tracking setup

1b
session_set_controller prepares VT
logind-session.c:1506
if (prepare) { r = session_prepare_vt(s);
session_open_vt()
fchown() for ownership
VT configuration sequence

1c
VT keyboard is disabled
logind-session.c:1339
r = ioctl(vt, KDSKBMODE, K_OFF);

1d
VT set to graphics mode
logind-session.c:1347
r = ioctl(vt, KDSETMODE, KD_GRAPHICS);

1e
VT process mode configured
logind-session.c:1358


## Example Mutter from Gnome

Motivation
When Mutter runs as a Wayland compositor on a Linux system, it needs to control the session to manage things like VT switching and device access. Without registering as the session controller, Mutter couldn't properly handle these system-level operations, leading to broken display switching and input device management.

Details
The registration happens during backend initialization when the native backend creates a launcher instance [1a]. This launcher obtains a D-Bus proxy to the systemd-logind session and calls the TakeControl method to register as the session controller [1b].

The process is:

Native backend starts up and creates a MetaLauncher
Launcher gets the session proxy from systemd-logind
Calls TakeControl D-Bus method with force=FALSE
Stores the result to track if control was acquired
If registration fails, Mutter continues running but with limited capabilities - it won't be able to switch VTs or manage device permissions. The launcher exposes the control status via meta_launcher_is_session_controller() so other components can check if Mutter successfully became the session controller.


## GDM Greeter

how does the greeter take over the seat when it starts (not how it communicates with systemd-logind to create a user session once the user enters his credentials.
Fast
The greeter takes over the seat through GDM's display management system when it starts. This process involves creating a display for the seat and launching the greeter session on that display.

Seat Takeover Process
1. Display Creation for Seat
When a greeter needs to start, ensure_display_for_seat() in daemon/gdm-local-display-factory.c is called to create a display for the specific seat gdm-local-display-factory.c:814-880 . This function:

Checks if the seat supports graphics via sd_seat_can_graphical() gdm-local-display-factory.c:863-876
Creates a new GdmDisplay object for the seat
Sets up the display environment
2. Greeter Session Startup
Once the display is created, gdm_display_start_greeter_session() in daemon/gdm-display.c starts the greeter process gdm-display.c:1682-1743 . This function:

Gets the seat ID and display name for the greeter gdm-display.c:1698-1708
Sets up signal handlers for the launch environment lifecycle gdm-display.c:1709-1728
Starts the greeter via gdm_launch_environment_start() gdm-display.c:1736
3. Manager Coordination
The GdmManager coordinates this process through set_up_greeter_session() which: gdm-manager.c:1432-1449

Determines the appropriate user for the greeter display
Creates a user session for the greeter
Calls gdm_display_start_greeter_session() to start the greeter
Seat Activation Mechanism
The greeter gains control of the seat through the display server (X11 or Wayland) that runs as part of the launch environment. When gdm_launch_environment_start() is called, it:

Starts the appropriate display server for the seat
Launches the greeter program on that display server
The display server provides the greeter with access to the seat's graphics hardware and input devices
This gives the greeter exclusive control of the seat's display and input devices until the user authenticates and a user session is started.

Notes
The seat takeover is essentially the process of creating a display-server instance for the seat and launching the greeter as a client of that display server. The greeter runs as the unprivileged "gdm" user but gets control of the seat through its connection to the display server that manages the seat's hardware resources.

