-- The commands of the page, once: the id, the title, the group and the keys each has. The list's
-- keys, the status line, the host's list of commands (and, with the next phases, the palette and
-- the card of keys) are made of this table, so that a key is written in one place. `native` are the
-- keys the page reads itself from the keyboard (the cursor's); they are listed and shown, but not
-- given to the host, which would take them from the field that has the keyboard.

local kit = require("../lib/kit")

local commands = {}

local TABLE = {
	{ id = "down", title = "Next issue", group = "Issues", keys = { "down", "j" }, native = true },
	{ id = "up", title = "Previous issue", group = "Issues", keys = { "up", "k" }, native = true },
	{ id = "first", title = "First issue", group = "Issues", keys = { "home" }, native = true },
	{ id = "last", title = "Last issue", group = "Issues", keys = { "end" }, native = true },
	{ id = "fold", title = "Fold the group", group = "Issues", keys = { "left" }, when = "list" },
	{ id = "unfold", title = "Open the group", group = "Issues", keys = { "right" }, when = "list" },
	{ id = "scope_mine", title = "Show my issues", group = "Issues", keys = { "1" } },
	{ id = "scope_all", title = "Show all issues", group = "Issues", keys = { "2" } },
	{ id = "filter", title = "Filter issues", group = "Linear", keys = { "/" } },
	{ id = "clear", title = "Clear the filter", group = "Linear", keys = { "escape" }, native = true },
	{ id = "refresh", title = "Ask Linear again", group = "Linear", keys = { "r" }, when = "keyed" },
}

-- The conditions, by name; page.lua gives them: what the page shows, never where the keyboard is.
commands.when = {}

function commands.all()
	return TABLE
end

function commands.get(id)
	for _, command in TABLE do
		if command.id == id then
			return command
		end
	end
	return nil
end

-- The words of the first key of `id`, or nil.
function commands.first(id)
	local command = commands.get(id)
	return command ~= nil and command.keys[1] or nil
end

-- The first key of `id` as this keyboard writes it.
function commands.label(id)
	local first = commands.first(id)
	return first ~= nil and kit.keys_of(first) or nil
end

-- The list for the library's `ui.commands`: what the host is given, each with its `when` made a
-- function.
function commands.for_library()
	local out = {}
	for _, command in TABLE do
		if not command.native then
			local name = command.when
			out[#out + 1] = {
				id = command.id,
				title = command.title,
				group = command.group,
				keys = command.keys,
				when = function()
					local when = name ~= nil and commands.when[name] or nil
					return name == nil or (when ~= nil and when())
				end,
			}
		end
	end
	return out
end

return commands
