#!/bin/sh
# Package maintainer script: stop a running ExcaliShot before files are installed or removed.
#
# The app is a single-instance tray app, so an old process left running after an upgrade keeps
# the new launch from starting: the new process just hands its arguments to the old one and
# exits. Stopping it here means the next start uses the freshly installed version.
pkill -x excalishot-app 2>/dev/null || true
exit 0
