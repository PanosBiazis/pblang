"""Source-hidden Reader: actual ordered observer, rapid shortcuts, Save/restart."""
import reader_appearance_native as harness
import reader_zoom_native as zoom
PREFIX=zoom.PREFIX.replace("time.sleep(.18);x('keyup',name);time.sleep(.22)","time.sleep(.001);x('keyup',name);time.sleep(.04)")
PREFIX=PREFIX.replace("time.sleep(.4);click(700,44)","""time.sleep(.4);click(700,44)
    descriptors=[p for p in Path('/proc/'+str(child.pid)+'/fd').iterdir() if 'memfd:pblang-window-input' in os.readlink(p)]
    assert len(descriptors)==1,'Reader must actually attach ordered input'
    assert descriptors[0].stat().st_mode & 0o777 == 0o600,'input memory is private'""")
JOURNEY=zoom.JOURNEY.replace("chord('equal');chord('0');shot('zoom-reset')","chord('equal');chord('KP_Insert');shot('zoom-reset')").replace('passed=True,session_zoom_no_save=True','passed=True,actual_ordered_input_memfd=True,input_memory_mode="0600",key_hold_ms=1,keypad_reset_verified=True,session_zoom_no_save=True')
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+zoom.CLEANUP
    harness.main()
