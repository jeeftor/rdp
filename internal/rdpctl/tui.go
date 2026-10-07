package rdpctl

import (
	"fmt"
	"strings"

	"charm.land/bubbles/v2/textinput"
	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
)

var (
	titleStyle    = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("212"))
	selectedStyle = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("86"))
	mutedStyle    = lipgloss.NewStyle().Foreground(lipgloss.Color("245"))
	errorStyle    = lipgloss.NewStyle().Foreground(lipgloss.Color("204"))
)

type tuiMode int

const (
	listMode tuiMode = iota
	formMode
	confirmDeleteMode
)

type tuiModel struct {
	store    *Store
	profiles []Profile
	cursor   int
	mode     tuiMode
	inputs   []textinput.Model
	focus    int
	status   string
	err      error
}

// RunTUI opens the interactive connection manager.
func RunTUI(store *Store) error {
	profiles, err := store.List()
	if err != nil {
		return err
	}
	model := tuiModel{store: store, profiles: profiles}
	program := tea.NewProgram(model)
	_, err = program.Run()
	return err
}

func (m tuiModel) Init() tea.Cmd { return nil }

func (m tuiModel) Update(message tea.Msg) (tea.Model, tea.Cmd) {
	if key, ok := message.(tea.KeyPressMsg); ok {
		switch m.mode {
		case listMode:
			return m.updateList(key)
		case formMode:
			return m.updateForm(key)
		case confirmDeleteMode:
			return m.updateDelete(key)
		}
	}
	if m.mode == formMode {
		var command tea.Cmd
		m.inputs[m.focus], command = m.inputs[m.focus].Update(message)
		return m, command
	}
	return m, nil
}

func (m tuiModel) updateList(key tea.KeyPressMsg) (tea.Model, tea.Cmd) {
	switch key.String() {
	case "q", "ctrl+c":
		return m, tea.Quit
	case "up", "k":
		if m.cursor > 0 {
			m.cursor--
		}
	case "down", "j":
		if m.cursor < len(m.profiles)-1 {
			m.cursor++
		}
	case "n":
		m.mode = formMode
		m.inputs = newInputs()
		m.focus = 0
		return m, m.inputs[0].Focus()
	case "s":
		if profile, ok := m.selected(); ok {
			if err := m.store.AddShortcut(profile.ID); err != nil {
				m.err = err
			} else {
				m.status = "Launcher created for " + profile.Name
				m.err = nil
			}
		}
	case "x":
		if profile, ok := m.selected(); ok {
			if err := m.store.RemoveShortcut(profile.ID); err != nil {
				m.err = err
			} else {
				m.status = "Launcher removed for " + profile.Name
				m.err = nil
			}
		}
	case "d":
		if _, ok := m.selected(); ok {
			m.mode = confirmDeleteMode
		}
	case "enter":
		if profile, ok := m.selected(); ok {
			if err := m.store.Launch(profile.ID); err != nil {
				m.err = err
			} else {
				m.status = "Closed RDP session for " + profile.Name
				m.err = nil
			}
		}
	}
	return m, nil
}

func (m tuiModel) updateForm(key tea.KeyPressMsg) (tea.Model, tea.Cmd) {
	switch key.String() {
	case "esc":
		m.mode = listMode
		return m, nil
	case "tab", "down":
		m.inputs[m.focus].Blur()
		m.focus = (m.focus + 1) % len(m.inputs)
		return m, m.inputs[m.focus].Focus()
	case "shift+tab", "up":
		m.inputs[m.focus].Blur()
		m.focus = (m.focus + len(m.inputs) - 1) % len(m.inputs)
		return m, m.inputs[m.focus].Focus()
	case "enter":
		monitors := m.inputs[3].Value()
		profile := Profile{Name: m.inputs[0].Value(), Host: m.inputs[1].Value(), User: m.inputs[2].Value(), Fullscreen: true, MultiMonitor: monitors != "", Monitors: monitors}
		created, err := m.store.Save(profile)
		if err != nil {
			m.err = err
			return m, nil
		}
		m.profiles = append(m.profiles, created)
		m.mode, m.status, m.err = listMode, "Saved "+created.Name, nil
		m.cursor = len(m.profiles) - 1
	}
	return m, nil
}

func (m tuiModel) updateDelete(key tea.KeyPressMsg) (tea.Model, tea.Cmd) {
	switch key.String() {
	case "y":
		profile, _ := m.selected()
		if err := m.store.Remove(profile.ID); err != nil {
			m.err = err
		} else {
			m.profiles = append(m.profiles[:m.cursor], m.profiles[m.cursor+1:]...)
			if m.cursor >= len(m.profiles) && m.cursor > 0 {
				m.cursor--
			}
			m.status, m.err = "Removed "+profile.Name, nil
		}
		m.mode = listMode
	case "n", "esc":
		m.mode = listMode
	}
	return m, nil
}

func (m tuiModel) View() tea.View {
	var body string
	switch m.mode {
	case formMode:
		body = m.formView()
	case confirmDeleteMode:
		profile, _ := m.selected()
		body = fmt.Sprintf("Remove %q and its generated launcher? [y/N]", profile.Name)
	default:
		body = m.listView()
	}
	if m.err != nil {
		body += "\n\n" + errorStyle.Render("Error: "+m.err.Error())
	}
	if m.status != "" {
		body += "\n\n" + mutedStyle.Render(m.status)
	}
	view := tea.NewView(titleStyle.Render("FreeRDP Connections") + "\n" + body)
	view.AltScreen = true
	return view
}

func (m tuiModel) listView() string {
	if len(m.profiles) == 0 {
		return mutedStyle.Render("No connections yet. Press n to create one.") + "\n\n" + mutedStyle.Render("n new • q quit")
	}
	rows := make([]string, 0, len(m.profiles))
	for index, profile := range m.profiles {
		row := fmt.Sprintf("  %s  %s@%s", profile.Name, profile.User, profile.Host)
		if index == m.cursor {
			row = selectedStyle.Render("› " + strings.TrimPrefix(row, "  "))
		}
		rows = append(rows, row)
	}
	return strings.Join(rows, "\n") + "\n\n" + mutedStyle.Render("↑/↓ select • enter connect • n new • s add launcher • x remove launcher • d delete • q quit")
}

func (m tuiModel) formView() string {
	return "New connection\n\n" + m.inputs[0].View() + "\n" + m.inputs[1].View() + "\n" + m.inputs[2].View() + "\n" + m.inputs[3].View() + "\n\n" + mutedStyle.Render("Monitor IDs are optional (example: 0,1). tab next field • enter save • esc cancel")
}

func (m tuiModel) selected() (Profile, bool) {
	if len(m.profiles) == 0 || m.cursor >= len(m.profiles) {
		return Profile{}, false
	}
	return m.profiles[m.cursor], true
}

func newInputs() []textinput.Model {
	inputs := make([]textinput.Model, 4)
	for index := range inputs {
		inputs[index] = textinput.New()
	}
	inputs[0].Placeholder, inputs[0].Prompt = "Lab Windows", "Name: "
	inputs[1].Placeholder, inputs[1].Prompt = "windows.example.invalid", "Host: "
	inputs[2].Placeholder, inputs[2].Prompt = "USERNAME", "User: "
	inputs[3].Placeholder, inputs[3].Prompt = "0,1", "Monitor IDs: "
	return inputs
}
