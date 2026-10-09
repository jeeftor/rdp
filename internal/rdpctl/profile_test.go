package rdpctl

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func testStore(directory string) *Store {
	return &Store{
		configDir:  filepath.Join(directory, "config", "connections"),
		dataDir:    filepath.Join(directory, "data", "applications"),
		executable: func() (string, error) { return "/opt/freerdp/bin/rdpctl", nil },
	}
}

func TestSaveAndListProfile(t *testing.T) {
	store := testStore(t.TempDir())
	profile, err := store.Save(Profile{Name: "Lab Windows", Host: "192.0.2.10", User: "alice", Fullscreen: true})
	if err != nil {
		t.Fatal(err)
	}
	if profile.ID != "lab-windows" {
		t.Fatalf("ID = %q", profile.ID)
	}
	profiles, err := store.List()
	if err != nil {
		t.Fatal(err)
	}
	if len(profiles) != 1 || profiles[0].Host != "192.0.2.10" {
		t.Fatalf("profiles = %#v", profiles)
	}
	info, err := os.Stat(store.profilePath(profile.ID))
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm() != 0600 {
		t.Fatalf("profile mode = %o", info.Mode().Perm())
	}
}

func TestSaveRejectsDuplicateProfile(t *testing.T) {
	store := testStore(t.TempDir())
	_, err := store.Save(Profile{Name: "Lab Windows", Host: "192.0.2.10", User: "alice"})
	if err != nil {
		t.Fatal(err)
	}
	_, err = store.Save(Profile{Name: "Lab Windows", Host: "192.0.2.11", User: "alice"})
	if err == nil {
		t.Fatal("Save accepted an existing profile")
	}
}

func TestShortcutLifecycle(t *testing.T) {
	store := testStore(t.TempDir())
	profile, err := store.Save(Profile{Name: "Lab Windows", Host: "192.0.2.10", User: "alice"})
	if err != nil {
		t.Fatal(err)
	}
	if err := store.AddShortcut(profile.ID); err != nil {
		t.Fatal(err)
	}
	contents, err := os.ReadFile(store.shortcutPath(profile.ID))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(contents), "X-RDPCTL-Managed=true") || !strings.Contains(string(contents), "launch lab-windows") {
		t.Fatalf("unexpected launcher: %s", contents)
	}
	if err := store.RemoveShortcut(profile.ID); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(store.shortcutPath(profile.ID)); !os.IsNotExist(err) {
		t.Fatalf("launcher still exists: %v", err)
	}
}

func TestRemoveRejectsUnmanagedLauncher(t *testing.T) {
	store := testStore(t.TempDir())
	profile, err := store.Save(Profile{Name: "Lab Windows", Host: "192.0.2.10", User: "alice"})
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(store.dataDir, 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(store.shortcutPath(profile.ID), []byte("[Desktop Entry]\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := store.RemoveShortcut(profile.ID); err == nil {
		t.Fatal("RemoveShortcut accepted an unmanaged launcher")
	}
}

func TestSoftwareRenderingRequiresX11AndPassesExplicitEnvironment(t *testing.T) {
	root := t.TempDir()
	store := testStore(root)
	executable := filepath.Join(root, "bin", "rdpctl")
	if err := os.MkdirAll(filepath.Dir(executable), 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "freerdp"), []byte("fixture"), 0700); err != nil {
		t.Fatal(err)
	}
	store.executable = func() (string, error) { return executable, nil }
	profile, err := store.Save(Profile{Name: "Software", Host: "host", User: "user", SoftwareRendering: true})
	if err != nil {
		t.Fatal(err)
	}
	calls := 0
	store.run = func(name string, args ...string) error {
		calls++
		if name != "env" || len(args) < 5 || args[0] != "SDL_VIDEO_DRIVER=x11" || args[1] != "SDL_VIDEODRIVER=x11" || args[2] != "SDL_RENDER_DRIVER=software" || args[3] != "SDL_FRAMEBUFFER_ACCELERATION=0" || args[4] != filepath.Join(root, "freerdp") {
			t.Fatalf("command %s %v", name, args)
		}
		return nil
	}
	t.Setenv("DISPLAY", "")
	if store.Launch(profile.ID) == nil || calls != 0 {
		t.Fatal("launch without X11 display accepted")
	}
	t.Setenv("DISPLAY", ":42")
	if err := store.Launch(profile.ID); err != nil {
		t.Fatal(err)
	}
	if calls != 1 {
		t.Fatalf("launch calls %d", calls)
	}
}

func TestSavedVideoModesAreUsedByTerminalLaunch(t *testing.T) {
	root := t.TempDir()
	store := testStore(root)
	executable := filepath.Join(root, "bin", "rdpctl")
	if err := os.MkdirAll(filepath.Dir(executable), 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "freerdp"), []byte("fixture"), 0700); err != nil {
		t.Fatal(err)
	}
	store.executable = func() (string, error) { return executable, nil }
	t.Setenv("DISPLAY", ":test")
	t.Setenv("WAYLAND_DISPLAY", "test")
	for _, mode := range []string{"x11-opengl", "wayland-opengl", "x11-opengles2", "wayland-opengles2", "x11-software"} {
		t.Run(mode, func(t *testing.T) {
			saved, err := store.Save(Profile{Name: mode, Host: "host", User: "user", VideoMode: mode, WorkingVideoModes: []string{mode}})
			if err != nil {
				t.Fatal(err)
			}
			loaded, err := store.Load(saved.ID)
			if err != nil || loaded.VideoMode != mode || len(loaded.WorkingVideoModes) != 1 {
				t.Fatalf("saved mode: %+v %v", loaded, err)
			}
			called := false
			store.run = func(name string, args ...string) error {
				called = true
				parts := strings.SplitN(mode, "-", 2)
				joined := strings.Join(args, " ")
				if name != "env" || !strings.Contains(joined, "SDL_VIDEO_DRIVER="+parts[0]) || !strings.Contains(joined, "SDL_RENDER_DRIVER="+parts[1]) {
					t.Fatalf("%s %v", name, args)
				}
				return nil
			}
			if err := store.Launch(saved.ID); err != nil || !called {
				t.Fatalf("launch: %v", err)
			}
		})
	}
}
