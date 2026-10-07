// Package rdpctl stores connection profiles and creates GNOME launchers.
package rdpctl

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"unicode"
)

const managedLauncherKey = "X-RDPCTL-Managed=true"

// Profile is a password-free FreeRDP connection definition.
type Profile struct {
	ID           string `json:"id"`
	Name         string `json:"name"`
	Host         string `json:"host"`
	User         string `json:"user"`
	Fullscreen   bool   `json:"fullscreen"`
	MultiMonitor bool   `json:"multi_monitor"`
	Monitors     string `json:"monitors,omitempty"`
}

// Store manages profiles below the current user's XDG directories.
type Store struct {
	configDir  string
	dataDir    string
	executable func() (string, error)
	run        func(string, ...string) error
}

// NewStore returns a store using standard XDG user directories.
func NewStore() *Store {
	home, _ := os.UserHomeDir()
	configHome := os.Getenv("XDG_CONFIG_HOME")
	if configHome == "" {
		configHome = filepath.Join(home, ".config")
	}
	dataHome := os.Getenv("XDG_DATA_HOME")
	if dataHome == "" {
		dataHome = filepath.Join(home, ".local", "share")
	}
	return &Store{
		configDir:  filepath.Join(configHome, "rdpctl", "connections"),
		dataDir:    filepath.Join(dataHome, "applications"),
		executable: os.Executable,
		run: func(name string, args ...string) error {
			command := exec.Command(name, args...)
			command.Stdin = os.Stdin
			command.Stdout = os.Stdout
			command.Stderr = os.Stderr
			return command.Run()
		},
	}
}

// List returns profiles ordered by display name.
func (s *Store) List() ([]Profile, error) {
	entries, err := os.ReadDir(s.configDir)
	if errors.Is(err, os.ErrNotExist) {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("read connection profiles: %w", err)
	}
	profiles := make([]Profile, 0, len(entries))
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".json" {
			continue
		}
		contents, err := os.ReadFile(filepath.Join(s.configDir, entry.Name()))
		if err != nil {
			return nil, fmt.Errorf("read profile %q: %w", entry.Name(), err)
		}
		var profile Profile
		if err := json.Unmarshal(contents, &profile); err != nil {
			return nil, fmt.Errorf("parse profile %q: %w", entry.Name(), err)
		}
		profiles = append(profiles, profile)
	}
	sort.Slice(profiles, func(i, j int) bool { return strings.ToLower(profiles[i].Name) < strings.ToLower(profiles[j].Name) })
	return profiles, nil
}

// Save validates and atomically writes a profile.
func (s *Store) Save(profile Profile) (Profile, error) {
	profile.Name = strings.TrimSpace(profile.Name)
	profile.Host = strings.TrimSpace(profile.Host)
	profile.User = strings.TrimSpace(profile.User)
	profile.Monitors = strings.TrimSpace(profile.Monitors)
	if profile.Name == "" || profile.Host == "" || profile.User == "" {
		return Profile{}, errors.New("name, host, and user are required")
	}
	if profile.ID == "" {
		profile.ID = slug(profile.Name)
	}
	if profile.ID == "" {
		return Profile{}, errors.New("name must contain a letter or number")
	}
	if err := os.MkdirAll(s.configDir, 0700); err != nil {
		return Profile{}, fmt.Errorf("create profile directory: %w", err)
	}
	contents, err := json.MarshalIndent(profile, "", "  ")
	if err != nil {
		return Profile{}, fmt.Errorf("encode profile: %w", err)
	}
	contents = append(contents, '\n')
	path := s.profilePath(profile.ID)
	if _, err := os.Stat(path); err == nil {
		return Profile{}, fmt.Errorf("connection %q already exists", profile.ID)
	} else if !errors.Is(err, os.ErrNotExist) {
		return Profile{}, fmt.Errorf("check existing profile: %w", err)
	}
	if err := writePrivateFile(path, contents); err != nil {
		return Profile{}, fmt.Errorf("write profile: %w", err)
	}
	return profile, nil
}

// Remove deletes a profile and only its managed launcher.
func (s *Store) Remove(id string) error {
	if _, err := s.Load(id); err != nil {
		return err
	}
	if err := s.RemoveShortcut(id); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	if err := os.Remove(s.profilePath(id)); err != nil {
		return fmt.Errorf("remove profile: %w", err)
	}
	return nil
}

// Load returns one profile by its stable ID.
func (s *Store) Load(id string) (Profile, error) {
	contents, err := os.ReadFile(s.profilePath(id))
	if errors.Is(err, os.ErrNotExist) {
		return Profile{}, fmt.Errorf("connection %q does not exist", id)
	}
	if err != nil {
		return Profile{}, fmt.Errorf("read profile: %w", err)
	}
	var profile Profile
	if err := json.Unmarshal(contents, &profile); err != nil {
		return Profile{}, fmt.Errorf("parse profile: %w", err)
	}
	return profile, nil
}

// AddShortcut creates or updates a launcher owned by rdpctl.
func (s *Store) AddShortcut(id string) error {
	profile, err := s.Load(id)
	if err != nil {
		return err
	}
	executable, err := s.executable()
	if err != nil {
		return fmt.Errorf("locate rdpctl: %w", err)
	}
	if err := os.MkdirAll(s.dataDir, 0700); err != nil {
		return fmt.Errorf("create launcher directory: %w", err)
	}
	contents := fmt.Sprintf("[Desktop Entry]\nType=Application\nVersion=1.0\nName=%s\nComment=Open %s with FreeRDP\nExec=%s launch %s\nTerminal=false\nCategories=Network;RemoteAccess;\n%s\nX-RDPCTL-Profile=%s\n", desktopValue(profile.Name), desktopValue(profile.Name), desktopArgument(executable), profile.ID, managedLauncherKey, profile.ID)
	if err := os.WriteFile(s.shortcutPath(id), []byte(contents), 0644); err != nil {
		return fmt.Errorf("write GNOME launcher: %w", err)
	}
	return nil
}

// RemoveShortcut deletes a launcher only when rdpctl generated it.
func (s *Store) RemoveShortcut(id string) error {
	path := s.shortcutPath(id)
	contents, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	if !strings.Contains(string(contents), managedLauncherKey) {
		return fmt.Errorf("refusing to remove unmanaged launcher %q", path)
	}
	if err := os.Remove(path); err != nil {
		return fmt.Errorf("remove GNOME launcher: %w", err)
	}
	return nil
}

// Launch starts the bundle's FreeRDP wrapper with one saved profile.
func (s *Store) Launch(id string) error {
	profile, err := s.Load(id)
	if err != nil {
		return err
	}
	executable, err := s.executable()
	if err != nil {
		return fmt.Errorf("locate rdpctl: %w", err)
	}
	resolved, err := filepath.EvalSymlinks(executable)
	if err == nil {
		executable = resolved
	}
	client := filepath.Join(filepath.Dir(filepath.Dir(executable)), "freerdp")
	if _, err := os.Stat(client); err != nil {
		return fmt.Errorf("bundled freerdp wrapper not found at %q: %w", client, err)
	}
	args := []string{"/v:" + profile.Host, "/u:" + profile.User}
	if profile.MultiMonitor {
		args = append(args, "/multimon")
	}
	if profile.Monitors != "" {
		args = append(args, "/monitors:"+profile.Monitors)
	}
	if profile.Fullscreen {
		args = append(args, "/f")
	}
	return s.run(client, args...)
}

func (s *Store) profilePath(id string) string { return filepath.Join(s.configDir, slug(id)+".json") }
func (s *Store) shortcutPath(id string) string {
	return filepath.Join(s.dataDir, "rdpctl-"+slug(id)+".desktop")
}

func writePrivateFile(path string, contents []byte) error {
	temporary, err := os.CreateTemp(filepath.Dir(path), ".profile-*.json")
	if err != nil {
		return err
	}
	temporaryName := temporary.Name()
	defer os.Remove(temporaryName)
	if err := temporary.Chmod(0600); err != nil {
		return err
	}
	if _, err := temporary.Write(contents); err != nil {
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	return os.Rename(temporaryName, path)
}

func slug(value string) string {
	var builder strings.Builder
	lastDash := false
	for _, character := range strings.ToLower(value) {
		if unicode.IsLetter(character) || unicode.IsDigit(character) {
			builder.WriteRune(character)
			lastDash = false
		} else if !lastDash && builder.Len() > 0 {
			builder.WriteByte('-')
			lastDash = true
		}
	}
	return strings.Trim(builder.String(), "-")
}

func desktopValue(value string) string {
	return strings.ReplaceAll(strings.ReplaceAll(value, "\n", " "), "\r", " ")
}

func desktopArgument(value string) string {
	return "\"" + strings.ReplaceAll(strings.ReplaceAll(value, "\\", "\\\\"), "\"", "\\\"") + "\""
}
