-- The head of the page, the whole width of it (what stands beside the list is under it): the list
-- that is shown (its name, its count, and the card of lists it opens), the filter (with how many it
-- lets through inside it), Refresh and New issue. Each is where it is and as wide as it is whatever
-- the page is doing: what is selected, what is under the cursor and the issue that is open change
-- nothing in it. The head is one row, the list at the left and the filter at the right (the same width
-- whatever the list is called: a long name is cut in its button); where the pane has no room for both
-- the filter has a row of its own. Refresh and New issue leave for the line of keys as the pane
-- narrows. Under them, where there is something to say, the row of the filters of the list
-- (`filters.lua`).

local kit = require("../lib/kit")
local commands = require("./commands")
local filters = require("./filters")
local list = require("./list")
local lists = require("./lists")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local head = {}

-- The field's state: what it holds.
local F = { value = "" }
head.field = F

local FIELD = u:name("filter", "field")

-- Puts the keyboard in the field, for `/`.
function head.focus()
	return u:focus(FIELD)
end

-- Puts the keyboard back on the page, where the list has the arrows. The library is told too
-- (`u:focus(nil)`): a reply alone moves the host's keyboard and leaves the library sending Enter
-- and Esc to the field.
function head.leave()
	return u:focus(nil)
end

-- Puts `text` in the field and tells the program.
function head.set(text)
	F.value = text
	act.view({ filter = text })
end

-- Whether the keyboard is in the field.
function head.typing()
	return u.holder == FIELD
end

-- The words of the field, with no space around them.
function head.words()
	return (string.match(F.value, "^%s*(.-)%s*$"))
end

-- The list the person was at when they searched, to go back to: `{ kind, id }`.
local before = nil

-- Searches all of Linear for the words of the field: the result is a list of its own, named Search,
-- and the words stay in the field. The keyboard goes back to the list. The reply.
function head.search()
	local words = head.words()
	if words == "" then
		return true
	end
	if S.scope ~= "search" then
		before = { kind = S.scope, id = S.list_id }
	end
	act.view({ list = { kind = "search", id = words } })
	return head.leave()
end

-- Leaves the search for the list that was before it, and empties the field. The reply.
function head.leave_search()
	local go = before or { kind = "mine" }
	before = nil
	F.value = ""
	act.view({ list = go, filter = "" })
	return head.leave()
end

-- Another list was gone to from a search: the words were the search's, and go with it.
act.field_clear = function()
	F.value = ""
	before = nil
end

-- Whether the field has words that nothing held matches (and it is not a search, whose words Linear
-- matched): the page says so, with the way to search all of Linear.
function head.unmatched()
	return S.scope ~= "search" and F.value ~= "" and S.issues ~= nil and S.issues.listing == "ready" and list.count() == 0
end

-- What the filter did. Whether it was the head's, and its reply.
function head.action(action)
	local from, kind = action.from, action.kind
	if from == "filter" then
		if kind == "change" then
			head.set(action.text)
		elseif kind == "submit" then
			-- With Command it is the search of all of Linear.
			if action.mods ~= nil and action.mods.command then
				return head.search()
			end
			-- Enter in the field is Enter on the list: the issue the cursor is on opens, and the
			-- keyboard goes back to the page.
			local item = list.current()
			if item ~= nil and item.kind == "issue" then
				act.open_issue(item)
			end
			return head.leave()
		elseif kind == "escape" then
			-- In a search the words are the search's: Esc goes back to the list that was before it. Else
			-- the first escape clears the field, the next leaves it.
			if S.scope == "search" then
				return head.leave_search()
			elseif F.value ~= "" then
				head.set("")
			else
				return head.leave()
			end
		end
		return true
	elseif from == "filter-clear" and kind == "press" then
		if S.scope == "search" then
			return head.leave_search()
		end
		head.set("")
		return head.focus()
	end
	return nil
end

-- How many open issues the list shown holds, as the button says it: "500+" where there are more than
-- the program takes. Nothing while the list has not come.
local function count_words(snapshot)
	local counts = snapshot ~= nil and snapshot.loaded and snapshot.listing == "ready" and snapshot.counts or nil
	if counts == nil then
		return nil
	end
	return tostring(counts.shown) .. (counts.shownCut and "+" or "")
end

-- The most of a list's name the button shows; a longer one is cut with the kit's mark.
local NAME_MOST = 26

-- The name of the list shown, cut where it is longer than the button has room for.
local function name_words()
	local name = lists.name()
	if utf8.len(name) > NAME_MOST then
		return string.sub(name, 1, utf8.offset(name, NAME_MOST) - 1) .. "\u{2026}"
	end
	return name
end

-- The list that is shown: its name and its count, and the mark of the card it opens.
local function lists_button()
	return kit.button("lists", name_words(), {
		bold = true,
		count = count_words(S.issues),
		chevron = true,
		tip = "Go to another list, key " .. (commands.label("lists") or "g"),
	})
end

-- "4 of 39" while the filter has words, what it lets through of what the list holds.
local function matched_words()
	local snapshot = S.issues
	-- (The words of a search were matched by Linear: it is not a count of what they let through.)
	if F.value == "" or S.scope == "search" or snapshot == nil or not snapshot.loaded then
		return nil
	end
	return string.format("%d of %d", snapshot.matched, snapshot.counts.shown)
end

-- The filter, a field of the same width whatever it holds: how many it lets through is inside it, at
-- its end, and so is what clears it. `width` is nil where the field has the whole of its row.
local function filter_box(width)
	local trail = {}
	local words = matched_words()
	if words ~= nil then
		trail[#trail + 1] = b.text(words):text_size(SIZE.small):color("dim"):no_shrink()
	end
	if F.value ~= "" then
		trail[#trail + 1] = u:icon_button("filter-clear", { icon = "close", label = "Clear the filter", keys = "escape" })
	elseif not head.typing() then
		trail[#trail + 1] = u:kbd(commands.first("filter"), { quiet = true })
	end
	local field = u:input("filter", {
		state = F,
		label = "Filter issues",
		hint = "Filter issues",
		trail = trail,
		escape = true,
	})
	local box = b.div():id("filter-box"):child(field)
	if width ~= nil then
		return box:w(width):no_shrink()
	end
	return box:grow(1):min_w(0)
end

-- The button that asks Linear again: busy while it does.
local function refresh_button()
	local busy = S.refreshing
	return kit.button("refresh", "Refresh", {
		suffix = not busy and commands.label("refresh") or nil,
		busy = busy,
		disabled = busy,
		tip = busy and "Linear is being asked" or "Ask Linear for the issues again",
	})
end

-- The button that opens the form of a new issue: in its place, with its key, while a form is open or
-- words are typed, as dim as a button that cannot be pressed now. Where the pane has less room the key
-- is in the line of keys.
local function new_button()
	-- (In a slot of its own: a button that cannot be pressed has no name of its own on the page, and its place
	-- is what is looked at.)
	return b.div():id("new-slot"):no_shrink():child(kit.button("new", "New issue", {
		suffix = commands.label("new"),
		disabled = S.form_open,
		tip = S.form_open and "A form is open" or "Write an issue and add it to Linear",
	}))
end

function head.view()
	local width = S.width
	local stacked = width < SIZE.stacked
	local bar = b.div():row():items_center():gap(10):pt(SIZE.head_top):pb(SIZE.head_bottom):px(SIZE.pad):no_shrink()
	bar:child(lists_button())
	bar:child(b.spacer())
	if not stacked then
		bar:child(filter_box(width >= SIZE.wide and SIZE.filter_wide or SIZE.filter))
	end
	if width >= SIZE.wide then
		bar:child(refresh_button())
	end
	if width >= SIZE.roomy then
		bar:child(new_button())
	end
	local box = b.div():col():no_shrink():child(bar)
	if stacked then
		box:child(b.div():px(SIZE.pad):pb(8):child(filter_box(nil)))
	end
	local row = filters.view()
	if row ~= nil then
		box:child(row)
	end
	return box
end

return head
