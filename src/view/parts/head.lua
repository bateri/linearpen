-- The head of the page, the whole width of it (what stands beside the list is under it): the list
-- that is shown (its name, its count, and the card of lists it opens), the filter (with how many it
-- lets through inside it), Refresh and New issue. Each is where it is and as wide as it is whatever
-- the page is doing: what is selected, what is under the cursor and the issue that is open change
-- nothing in it. Where the pane is narrow the filter has a row of its own and Refresh and New issue
-- leave for the line of keys.

local kit = require("../lib/kit")
local commands = require("./commands")
local list = require("./list")

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

-- What the filter did. Whether it was the head's, and its reply.
function head.action(action)
	local from, kind = action.from, action.kind
	if from == "filter" then
		if kind == "change" then
			head.set(action.text)
		elseif kind == "submit" then
			-- Enter in the field is Enter on the list: the issue the cursor is on opens, and the
			-- keyboard goes back to the page.
			local item = list.current()
			if item ~= nil and item.kind == "issue" then
				act.open_issue(item)
			end
			return head.leave()
		elseif kind == "escape" then
			-- The first escape clears the field, the next leaves it.
			if F.value ~= "" then
				head.set("")
			else
				return head.leave()
			end
		end
		return true
	elseif from == "filter-clear" and kind == "press" then
		head.set("")
		return head.focus()
	end
	return nil
end

-- How many open issues a list holds, as the button says it: "500+" where there are more than the
-- program takes.
local function count_words(snapshot, scope)
	local counts = snapshot ~= nil and snapshot.loaded and snapshot.counts or nil
	if counts == nil then
		return nil
	end
	local cut = scope == "mine" and counts.mineCut or scope == "all" and counts.allCut
	return tostring(scope == "mine" and counts.mine or counts.all) .. (cut and "+" or "")
end

-- The list that is shown: its name and its count, and the mark of the card it opens.
local function lists_button()
	return kit.button("lists", S.scope == "all" and "All issues" or "My issues", {
		bold = true,
		count = count_words(S.issues, S.scope),
		chevron = true,
		tip = "Go to another list, key " .. (commands.label("lists") or "g"),
	})
end

-- "4 of 39" while the filter has words, what it lets through of what the list holds.
local function matched_words()
	local snapshot = S.issues
	if F.value == "" or snapshot == nil or not snapshot.loaded then
		return nil
	end
	local total = S.scope == "mine" and snapshot.counts.mine or snapshot.counts.all
	return string.format("%d of %d", snapshot.matched, total)
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

-- Whether the keys of the buttons are left out of them: where words are being written, and where a
-- form has the page.
local function keyless()
	return head.typing() or S.form_open
end

-- The button that asks Linear again: busy while it does.
local function refresh_button()
	local busy = S.refreshing
	return kit.button("refresh", "Refresh", {
		suffix = (not busy and not keyless()) and commands.label("refresh") or nil,
		busy = busy,
		disabled = busy,
		tip = busy and "Linear is being asked" or "Ask Linear for the issues again",
	})
end

-- The button that opens the form of a new issue: in its place while a form is open, as dim as a
-- button that cannot be pressed now. Where the pane has less room the key is in the line of keys.
local function new_button()
	return kit.button("new", "New issue", {
		suffix = not keyless() and commands.label("new") or nil,
		disabled = S.form_open,
		tip = S.form_open and "A form is open" or "Write an issue and add it to Linear",
	})
end

function head.view()
	local width = S.width
	local narrow = width < SIZE.narrow
	local bar = b.div():row():items_center():gap(10):pt(SIZE.head_top):pb(SIZE.head_bottom):px(SIZE.pad):no_shrink()
	bar:child(lists_button())
	bar:child(b.spacer())
	if not narrow then
		bar:child(filter_box(width >= SIZE.wide and SIZE.filter_wide or SIZE.filter))
	end
	if width >= SIZE.wide then
		bar:child(refresh_button())
	end
	if width >= SIZE.roomy then
		bar:child(new_button())
	end
	if not narrow then
		return bar
	end
	local box = b.div():col():no_shrink():child(bar)
	box:child(b.div():px(SIZE.pad):pb(8):child(filter_box(nil)))
	return box
end

return head
