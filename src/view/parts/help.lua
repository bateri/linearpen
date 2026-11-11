-- The card of keys: every command with the keys it has, in groups, from the same table as the palette
-- and the status line. A modal over the page; Esc closes it.

local kit = require("../lib/kit")
local commands = require("./commands")

local b, u, S, act = kit.b, kit.u, kit.S, kit.act

local help = {}

local H = { open = false }

-- The box of the card that scrolls.
local SCROLL = "help/scroll"

-- The groups of the card, in the order they stand in the table.
local ORDER = { "Move", "Select", "Change", "Issue", "Work on, New issue", "Everywhere" }

function help.is_open()
	return H.open
end

function help.show()
	H.open = true
end

function help.hide()
	H.open = false
end

local function label(words)
	return b.text(string.upper(words)):text_size(11.5):color("dim")
end

-- The keys of a command, as caps, and its words. Three keys at most: the rest are in the table.
local function line(command, title)
	local row = b.div():row():items_center():gap(8):h(22):no_shrink()
	for index, words in command.keys do
		if index > 3 then
			break
		end
		row:child(u:kbd(words))
	end
	row:child(b.text(title):text_size(13):truncate():min_w(0):grow(1))
	return row
end

-- Whether two commands have the same keys, in the same order.
local function same_keys(a, b_)
	if #a.keys ~= #b_.keys then
		return false
	end
	for index, words in a.keys do
		if b_.keys[index] ~= words then
			return false
		end
	end
	return true
end

-- The commands of a group, a key said once: commands that have the same keys (the key that starts the
-- agent is the one that creates the issue) are one line, "Start the agent, create the issue".
local function group(name)
	local box = b.div():id("help/group/" .. name):col():gap(7):mb(18)
	box:child(label(name))
	local lines = {}
	for _, command in commands.all() do
		if command.group == name then
			local last = lines[#lines]
			if last ~= nil and same_keys(last.command, command) then
				last.title = last.title .. ", " .. string.lower(string.sub(command.title, 1, 1)) .. string.sub(command.title, 2)
			else
				lines[#lines + 1] = { command = command, title = command.title }
			end
		end
	end
	for _, each in lines do
		box:child(line(each.command, each.title))
	end
	return box
end

function help.view(root)
	if not H.open then
		return
	end
	local width = math.min(640, S.width - 32)
	local inner = width - 36
	local body = b.div():col():p(16):pb(0):w(width)
	local head = b.div():row():items_center():gap(8):mb(16):w(inner)
	head:child(b.text("Keys"):text_size(15):weight(700))
	head:child(b.spacer())
	head:child(kit.button("help-close", "Close", { suffix = "Esc" }))
	body:child(head)
	local columns = b.div():row():items_start():gap(28):w(inner)
	if width >= 620 then
		-- The left groups have short words; the room they leave is the right's, whose longest line
		-- is whole. A narrower card has one column, so that no line of it is cut.
		local left_width = 210
		local left = b.div():col():w(left_width):no_shrink()
		local right = b.div():col():w(inner - 28 - left_width):no_shrink()
		for index, name in ORDER do
			local side = index <= 3 and left or right
			side:child(group(name))
		end
		columns:child(left):child(right)
	else
		local single = b.div():col():w(inner):no_shrink()
		for _, name in ORDER do
			single:child(group(name))
		end
		columns:child(single)
	end
	body:child(b.div():id(SCROLL):scroll():max_h(math.max(S.height - 160, 160)):w(inner):child(columns))
	root:child(u:modal("help", { width = width, label = "the keys", top = 40 }, body))
end

-- A key while the card is open: the arrows and Page Up and Page Down scroll a card taller than the
-- pane, which the keyboard could not read to its end otherwise. (Space is the button's that has the
-- keyboard, Close.) Whether the key was the card's.
local STEPS = { down = 1, j = 1, up = -1, k = -1 }
local PAGES = { page_down = 1, page_up = -1 }

function help.key(event)
	if not H.open then
		return nil
	end
	local mods = event.mods or {}
	if mods.command or mods.control or mods.alt or mods.shift then
		return nil
	end
	local extent = b.layout.scroll_extent(SCROLL)
	local room = extent ~= nil and extent.view or 0
	local step = STEPS[event.key]
	local by = step ~= nil and step * 44 or nil
	if by == nil and PAGES[event.key] ~= nil then
		by = PAGES[event.key] * math.max(room - 44, 44)
	end
	if by == nil then
		return nil
	end
	kit.scroll_to(SCROLL, (kit.offset(SCROLL) or 0) + by, true)
	return true
end

function help.action(action)
	if action.from == "help" and action.kind == "dismiss" then
		H.open = false
		return true
	end
	return nil
end

act.open_help = help.show

return help
