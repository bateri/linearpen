-- The palette: a field over every command that can be done now (with its keys), and what is typed
-- as the words of a filter: `Filter issues by "bug"`. From the same table as the card of keys and the
-- status line.

local fuzzy = require("@pencere/fuzzy")
local kit = require("../lib/kit")
local commands = require("./commands")
local head = require("./head")

local u, S, act = kit.u, kit.S, kit.act

local palette = {}

-- `filters` is whether the words typed can be a filter: the head is on the page.
local P = { open = false, items = nil, state = nil, serial = 0, filters = true }
palette.state = P

local LIMIT = 40

-- What the person typed as the words of the filter.
-- (In the list of projects the words filter the projects by their names.)
local function as_filter(query)
	local what = S.scope == "projects" and "Projects" or "Issues"
	return {
		key = "go:" .. query,
		title = string.format("Filter %s by “%s”", string.lower(what), query),
		hint = what,
		text = query,
	}
end

-- The page's own way of finding: the commands that fuzzily match, then the query as a filter (where
-- the head is on the page: where the issue or a form has the pane, the filter is not there to take it).
local function found(query, items)
	local rows = {}
	if query == "" then
		for index = 1, math.min(#items, 200) do
			rows[#rows + 1] = items[index]
		end
		return rows
	end
	for _, match in fuzzy.filter(query, items, { text = "title", limit = LIMIT }) do
		rows[#rows + 1] = match.item
	end
	if P.filters then
		rows[#rows + 1] = as_filter(query)
	end
	return rows
end

function palette.is_open()
	return P.open
end

function palette.show()
	-- The commands that can be done now, made as the palette opens (while the page has the keyboard,
	-- which is what most of them need) and kept until it closes.
	P.items = commands.items()
	P.filters = not S.covered
	-- A palette of its own each time: the host keeps the text of a field of the same name, and one
	-- opened again would begin with the words of the last.
	P.serial += 1
	P.state = { query = "", cursor = nil, field = { value = "" } }
	P.open = true
end

function palette.hide()
	P.open = false
end

function palette.view(root)
	if not P.open then
		return
	end
	root:child(u:palette("palette" .. P.serial, {
		items = P.items,
		state = P.state,
		label = "Command palette",
		hint = P.filters and ("Find a command, or filter the " .. (S.scope == "projects" and "projects" or "issues"))
			or "Find a command",
		filter = found,
		rows = kit.card_rows(),
		footer = {
			{ keys = "enter", title = "Do it" },
			{ keys = "escape", title = "Close" },
		},
	}))
end

-- An action of the palette: whether it was one, and its reply.
function palette.action(action)
	if type(action.from) ~= "string" or string.match(action.from, "^palette%d+$") == nil then
		return nil
	end
	if action.kind == "dismiss" then
		P.open = false
		return true
	elseif action.kind == "query" then
		-- The best match is the one the cursor goes to, as the query changes.
		P.state.cursor = nil
		return true
	elseif action.kind == "pick" then
		P.open = false
		local item = action.item
		if item.text ~= nil then
			head.set(item.text)
			return true
		end
		return act.run(item.key)
	end
	return true
end

return palette
