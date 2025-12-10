# Security

## Small code base

## Fuzzing

## Audit

## Unsafe code

Known problem: A lot of unsafe code. Open are ideas to mitigate this issue.

## seccomp, AppArmor, SELinux, cgroups mounts,  /sys read-write

This is a big TODO. Which permissions can be reduced. Now we assume we are quite privileagued:
- We have all Linux kernel capabilities,
- The default seccomp profile is disabled,
- The default AppArmor profile is disabled,
- The default SELinux process label is disabled,
- all host devices are accessible,
- /sys is read-write,
- cgroups mount is read-write.