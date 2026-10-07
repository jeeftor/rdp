// Command rdpctl manages password-free FreeRDP connection profiles and GNOME launchers.
package main

import (
	"fmt"
	"os"

	"github.com/jeeftor/rdp/internal/rdpctl"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

func main() {
	if err := newRootCommand().Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func newRootCommand() *cobra.Command {
	viper.SetEnvPrefix("RDPCTL")
	viper.AutomaticEnv()

	store := rdpctl.NewStore()
	root := &cobra.Command{
		Use:   "rdpctl",
		Short: "Manage portable FreeRDP connections and GNOME launchers",
		Args:  cobra.NoArgs,
		RunE: func(_ *cobra.Command, _ []string) error {
			return rdpctl.RunTUI(store)
		},
	}

	root.AddCommand(newListCommand(store), newAddCommand(store), newRemoveCommand(store), newShortcutCommand(store), newLaunchCommand(store))
	return root
}

func newListCommand(store *rdpctl.Store) *cobra.Command {
	return &cobra.Command{
		Use:   "list",
		Short: "List saved connections",
		Args:  cobra.NoArgs,
		RunE: func(_ *cobra.Command, _ []string) error {
			profiles, err := store.List()
			if err != nil {
				return err
			}
			for _, profile := range profiles {
				fmt.Printf("%s\t%s\t%s\n", profile.ID, profile.Name, profile.Host)
			}
			return nil
		},
	}
}

func newAddCommand(store *rdpctl.Store) *cobra.Command {
	var profile rdpctl.Profile
	command := &cobra.Command{
		Use:   "add",
		Short: "Save a connection profile",
		Args:  cobra.NoArgs,
		RunE: func(_ *cobra.Command, _ []string) error {
			created, err := store.Save(profile)
			if err != nil {
				return err
			}
			fmt.Printf("Saved %q as %s.\n", created.Name, created.ID)
			return nil
		},
	}
	command.Flags().StringVar(&profile.Name, "name", "", "Display name")
	command.Flags().StringVar(&profile.Host, "host", "", "RDP server hostname or IP address")
	command.Flags().StringVar(&profile.User, "user", "", "RDP username")
	command.Flags().BoolVar(&profile.Fullscreen, "fullscreen", true, "Launch full screen")
	command.Flags().BoolVar(&profile.MultiMonitor, "multimon", false, "Use multiple monitors")
	command.Flags().StringVar(&profile.Monitors, "monitors", "", "Comma-separated monitor IDs")
	_ = command.MarkFlagRequired("name")
	_ = command.MarkFlagRequired("host")
	_ = command.MarkFlagRequired("user")
	return command
}

func newRemoveCommand(store *rdpctl.Store) *cobra.Command {
	var confirmed bool
	command := &cobra.Command{
		Use:   "remove PROFILE",
		Short: "Remove a profile and its generated launcher",
		Args:  cobra.ExactArgs(1),
		RunE: func(_ *cobra.Command, args []string) error {
			if !confirmed {
				return fmt.Errorf("refusing to remove %q without --yes", args[0])
			}
			if err := store.Remove(args[0]); err != nil {
				return err
			}
			fmt.Printf("Removed %s and its generated launcher.\n", args[0])
			return nil
		},
	}
	command.Flags().BoolVar(&confirmed, "yes", false, "Confirm profile removal")
	return command
}

func newShortcutCommand(store *rdpctl.Store) *cobra.Command {
	command := &cobra.Command{Use: "shortcut", Short: "Manage GNOME application launchers"}
	command.AddCommand(
		&cobra.Command{Use: "add PROFILE", Short: "Create a GNOME launcher", Args: cobra.ExactArgs(1), RunE: func(_ *cobra.Command, args []string) error {
			if err := store.AddShortcut(args[0]); err != nil {
				return err
			}
			fmt.Printf("Created GNOME launcher for %s.\n", args[0])
			return nil
		}},
		&cobra.Command{Use: "remove PROFILE", Short: "Remove the generated GNOME launcher", Args: cobra.ExactArgs(1), RunE: func(_ *cobra.Command, args []string) error {
			if err := store.RemoveShortcut(args[0]); err != nil {
				return err
			}
			fmt.Printf("Removed GNOME launcher for %s.\n", args[0])
			return nil
		}},
	)
	return command
}

func newLaunchCommand(store *rdpctl.Store) *cobra.Command {
	return &cobra.Command{
		Use:   "launch PROFILE",
		Short: "Launch a saved profile with the bundled FreeRDP client",
		Args:  cobra.ExactArgs(1),
		RunE: func(_ *cobra.Command, args []string) error {
			return store.Launch(args[0])
		},
	}
}
