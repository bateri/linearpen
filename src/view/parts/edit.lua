-- The edit of the words of an issue: its description, or a comment of the person's (Linear lets only the
-- one who wrote a comment edit it). Edit description and Edit comment ask the program for the text whole
-- as Linear holds it now (`edit-open`: what the page shows of it is cut, and is never what is edited),
-- and the place of the words becomes a field that holds it. Save sends the person's words
-- (`edit-save`), which the program writes only where Linear holds what the edit began from; Cancel lets
-- them go. One edit is open at a time on an issue; its words are kept while the issue is not on the page
-- (another issue, a form), as a comment's are, and are never sent unless the person presses Save.

local kit = require("../lib/kit")
local attach = require("./attach")
local commands = require("./commands")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local edit = {}

-- The most characters an edit takes: the program says no to more.
local MOST = 100000

-- What the page holds: the edit open on each issue, by its id (`{ target, status, serial, saving,
-- problem, base }`: `target` is "description" or the id of the comment, `status` is "loading" until the
-- program has given the text and "ready" after, `base` is that text, `problem` why the last Save was
-- not done), why an edit could not begin (`failed`, by issue: `{ target, words }`), the words of each
-- edit as the person left them (`drafts`, by issue), the issue the field drawn last is of (`drawn`),
-- the issue whose words are being put into a field just made (`loading`), and the version of each
-- field as it was last read.
-- `reveal` is the issue whose field was just made, which the room the issue is read in is scrolled to
-- show once it is laid out (`settle`).
-- `asked` numbers the menu that asks before words are let go (a pick of an older one is not one).
local E = { open = {}, failed = {}, drafts = {}, drawn = nil, loading = nil, seen = {}, serial = 0, reveal = nil, asked = 0 }
edit.state = E

local function editor_name(id)
	local open = E.open[id]
	return "edit-" .. id .. "-" .. (open ~= nil and open.serial or 0)
end

-- The words the field of the issue `id` holds now, or nil where it is not on the page.
local function words_of(id)
	local name = editor_name(id)
	local length = b.layout.editor_length(name)
	if length == nil then
		return nil
	end
	return b.layout.editor_text(name, 1, length + 1) or ""
end

local function trimmed(words)
	return string.match(words, "^%s*(.-)%s*$") or ""
end

-- The edit open on the issue `id`, where it is of `target` ("description" or a comment's id; any, where
-- none is named), else nil.
function edit.of(id, target)
	local open = id ~= nil and E.open[id] or nil
	if open == nil or (target ~= nil and open.target ~= target) then
		return nil
	end
	return open
end

-- Whether the field of an edit is on the page with its words: the edit is ready.
local function ready(id)
	local open = E.open[id]
	return open ~= nil and open.status == "ready"
end

-- Keeps the words of the field drawn last, as they are: called before every event is done, while the
-- field is on the page still (an event may take it away), so that what was written is never lost to a
-- change of the page. Nothing is read while the field has not changed.
function edit.keep()
	local id = E.drawn
	if id == nil or not ready(id) then
		return
	end
	local name = editor_name(id)
	local version = b.layout.editor_version(name)
	-- (A version below the one last read is another document's: the field was made again, and is not read
	-- until it has said it is ready, which is when its kept words go in.)
	if version == nil or (E.seen[name] ~= nil and version <= E.seen[name]) then
		return
	end
	local words = words_of(id)
	if words == nil then
		return
	end
	if E.loading == id then
		-- The kept words are not in the field yet: what it holds is not them.
		if words == "" and (E.drafts[id] or "") ~= "" then
			return
		end
		E.loading = nil
	end
	E.seen[name] = version
	E.drafts[id] = words
end

-- The field was just made: the words kept of its edit are put in it, with the caret after them. The
-- reply, or nil.
function edit.ready(event)
	local id = E.drawn
	if id == nil or event.editor ~= editor_name(id) then
		return nil
	end
	E.seen[event.editor] = nil
	local kept = E.drafts[id]
	if kept == nil or kept == "" then
		E.loading = nil
		return nil
	end
	return { editor = { [event.editor] = { { load = kept }, { command = "move", movement = "document_end" } } } }
end

-- The field changed: a frame is drawn, for what is in it says whether Save can be pressed. The reply, or
-- nil.
function edit.edited(event)
	if E.drawn ~= nil and event.editor == editor_name(E.drawn) then
		return true
	end
	return nil
end

-- Whether the keyboard is in the field of an edit.
function edit.typing()
	return E.drawn ~= nil and ready(E.drawn) and u:keyboard() == editor_name(E.drawn)
end

-- Whether the keyboard is in the field of the edit of a description (files go to it then).
function edit.typing_description()
	return edit.typing() and E.open[E.drawn].target == "description"
end

-- The words that would be saved for the issue `id` (what the field holds, else what was kept).
local function words_for(id)
	local live = words_of(id)
	if live ~= nil and E.loading ~= id then
		return live
	end
	return E.drafts[id] or ""
end

-- The key of the files that go at the end of the description of the issue `id` (`attach.lua`'s).
local function files_of(id)
	return "describe:" .. id
end

-- Why Save cannot be pressed for the issue `id`, or nil where it can.
local function why_not(id)
	local open = E.open[id]
	if open == nil or open.status ~= "ready" or open.saving then
		return ""
	end
	local words = words_for(id)
	local length = utf8.len(words)
	if length == nil or length > MOST then
		return "It is up to " .. MOST .. " characters"
	end
	if open.target ~= "description" and trimmed(words) == "" then
		return "A comment cannot be empty"
	end
	return nil
end

function edit.saveable(id)
	return id ~= nil and why_not(id) == nil
end

-- Begins the edit of `target` ("description", or the id of a comment of the person's) of the issue `id`:
-- the program is asked for the text whole, and the place of the words becomes a field that holds it. The
-- reply.
function edit.begin(id, target)
	if id == nil or E.open[id] ~= nil then
		return true
	end
	E.serial += 1
	local serial = E.serial
	E.failed[id] = nil
	E.open[id] = { target = target, status = "loading", serial = serial }
	local params = { issue = id }
	if target ~= "description" then
		params.comment = target
	end
	kit.call("edit-open", params, function(result, err)
		local open = E.open[id]
		if open == nil or open.serial ~= serial then
			return true
		end
		if err then
			E.open[id] = nil
			E.failed[id] = { target = target, words = err.message or err.code }
			return true
		end
		open.status, open.base = "ready", result.text
		E.drafts[id] = result.text
		E.reveal = id
		-- The field has the keyboard where its issue is the one open.
		if act.panel_id() == id then
			return u:focus(u:field(editor_name(id)))
		end
		return true
	end)
	return true
end

-- Lets go of the edit of the issue `id` and of what it held: its words, and the files chosen for the
-- description. What Linear holds is asked for again (someone may have changed it). The reply.
function edit.cancel(id)
	local open = id ~= nil and E.open[id] or nil
	if open == nil or open.saving then
		return true
	end
	local had = edit.typing() and E.drawn == id
	E.open[id] = nil
	E.drafts[id] = nil
	E.failed[id] = nil
	if E.loading == id then
		E.loading = nil
	end
	if open.target == "description" then
		attach.clear(files_of(id))
	end
	if open.status == "ready" then
		act.panel_reask(id, function() end)
	end
	return had and u:focus(nil) or true
end

-- The menu that asks before an edit that holds what Linear does not is let go.
local MENU = "edit-discard"

-- Whether the edit of the issue `id` holds what Linear does not: words other than it began from, or
-- files chosen for the description.
local function changed(id)
	local open = E.open[id]
	if open == nil or open.status ~= "ready" then
		return false
	end
	if words_for(id) ~= open.base then
		return true
	end
	return open.target == "description" and attach.count(files_of(id)) > 0
end

-- Cancel, and Esc where it is Cancel's: an edit that holds nothing new is let go; one whose words were
-- changed asks first, by Cancel (the host's menu: Discard the edit, Keep editing), for a press of Esc is
-- easily one too many. The reply.
function edit.ask_cancel(id)
	local open = id ~= nil and E.open[id] or nil
	if open == nil or open.saving then
		return true
	end
	if not changed(id) then
		return edit.cancel(id)
	end
	E.asked += 1
	local items = {
		{ key = E.asked .. "|discard|" .. id, title = "Discard the edit" },
		{ key = E.asked .. "|keep|" .. id, title = "Keep editing" },
	}
	local where = { at = { x = S.width / 2 - 100, y = S.height / 3 } }
	if b.layout.bounds("edit-cancel") ~= nil then
		where = { near = "edit-cancel" }
	end
	return u:menu(MENU, items, where) or true
end

-- What a pick of that menu chose. The reply, or nil where it was not the menu's.
function edit.action(action)
	if action.from ~= MENU or action.kind ~= "menu" then
		return nil
	end
	local asked, what, id = string.match(action.entry or "", "^(%d+)|(%a+)|(.+)$")
	if asked == nil or tonumber(asked) ~= E.asked then
		return true
	end
	if what == "discard" then
		return edit.cancel(id)
	end
	-- Kept: the keyboard goes back to the words, where they are the issue open's.
	if ready(id) and act.panel_id() == id then
		return edit.focus(id)
	end
	return true
end

-- Sends the words of the edit of the issue `id`, and for a description the files chosen for it. The
-- field stays as it is until the program says they were saved. The reply.
function edit.save(id)
	if not edit.saveable(id) then
		return true
	end
	local open = E.open[id]
	local words = words_for(id)
	local files = open.target == "description" and attach.params(files_of(id)) or nil
	if words == open.base and files == nil then
		-- Nothing changed: there is nothing to write.
		return edit.cancel(id)
	end
	open.saving, open.problem = true, nil
	local params = { issue = id, text = words, files = files }
	if open.target ~= "description" then
		params.comment = open.target
	end
	kit.call("edit-save", params, function(result, err)
		local now = E.open[id]
		if now == nil or now.serial ~= open.serial then
			return true
		end
		now.saving = false
		if err then
			now.problem = err.message or err.code
			return true
		end
		-- Saved: the words are Linear's, and the issue is shown as it holds it now.
		local had = edit.typing() and E.drawn == id
		E.open[id] = nil
		E.drafts[id] = nil
		if E.loading == id then
			E.loading = nil
		end
		if files ~= nil then
			attach.release(files_of(id), files)
		end
		if result ~= nil and result.issue ~= nil then
			act.panel_put(id, result.issue)
		elseif not act.panel_reask(id, function() end) then
			act.panel_forget(id)
		end
		return had and u:focus(nil) or true
	end)
	return true
end

-- Once the field just made is laid out, the room the issue is read in is scrolled as far as shows it with
-- Save under it, and no further: the top of the field stays in view. Whether the page needs another frame.
function edit.settle()
	local id = E.reveal
	if id == nil then
		return false
	end
	if act.panel_id() ~= id or not ready(id) then
		E.reveal = nil
		return false
	end
	-- (The top is the line of the comment, where a comment is edited: who wrote it is seen with it.)
	local box = b.layout.bounds("edit-top") or b.layout.bounds("edit-box")
	local save, view = b.layout.bounds("edit-save"), act.panel_view()
	if box == nil or save == nil or view == nil then
		-- (The next frame lays it out.)
		return false
	end
	E.reveal = nil
	local at = act.panel_scroll() or 0
	local over = save.y + save.h + 16 - (view.y + view.h)
	local by = 0
	if box.y < view.y then
		by = box.y - view.y - 16
	elseif over > 0 then
		by = math.min(over, box.y - view.y - 16)
	end
	if by ~= 0 then
		act.panel_scroll(math.max(at + by, 0))
		return true
	end
	return false
end

-- Whether Esc lets go of the edit of the issue open: one is open, the keyboard is not in its field (Esc
-- there leaves the field, with the words), and it is not being saved. Then it is Cancel's, written on
-- Cancel.
function edit.cancelable()
	local id = act.panel_id()
	local open = id ~= nil and E.open[id] or nil
	return open ~= nil and not open.saving and not edit.typing()
end

-- Gives the keyboard to the field of the edit of the issue `id`. The reply.
function edit.focus(id)
	return u:focus(u:field(editor_name(id)))
end

-- A key while the field has the keyboard: Esc takes it back (the words stay), Tab goes to Save where it
-- can be pressed; every other key is the field's. The reply, or nil.
function edit.key(event)
	if not edit.typing() then
		return nil
	end
	local mods = event.mods or {}
	if mods.command or mods.control or mods.alt then
		return nil
	end
	if event.key == "escape" and not mods.shift then
		return u:focus(nil)
	elseif event.key == "tab" then
		if not mods.shift and edit.saveable(E.drawn) then
			return u:focus("edit-save")
		end
		return true
	end
	return nil
end

-- The last comment of the person's on the issue `detail`, or nil.
function edit.last_mine(detail)
	if detail == nil then
		return nil
	end
	local last = nil
	for _, one in detail.comments or {} do
		if one.mine and one.id ~= nil and one.id ~= "" then
			last = one
		end
	end
	return last
end

-- --- Drawing -----------------------------------------------------------------------------------------

-- How high the field is for `words` in `room` points across: as high as the words, held between `least`
-- and most of the room the issue is read in, which also has what is under the field to Save (`below`)
-- and what is over it (the line of a comment).
local function height(words, room, least, below)
	local across = math.max(math.floor((room - 24) / 8.1), 20)
	local lines = 0
	for line in string.gmatch(words .. "\n", "([^\n]*)\n") do
		lines += math.max(1, math.ceil((utf8.len(line) or #line) / across))
	end
	-- (Save is under it, and is seen with it: in a pane of few lines the field is what room is left.)
	local view = act.panel_view()
	local most = math.max(48, (view ~= nil and view.h or 400) - below - 40)
	return math.min(math.max(lines * 20 + 20, math.min(least, most)), most)
end

-- What the edit says where it is not done: why it was not saved, or that it is being.
local function said(id)
	local open = E.open[id]
	if open.saving and open.target == "description" and attach.count(files_of(id)) > 0 then
		return b.text(attach.uploading(files_of(id))):text_size(SIZE.small):color("dim"):grow(1)
	end
	if open.problem ~= nil then
		return b.div():row():items_start():gap(8):grow(1):min_w(0):child(
			b.div():size(7, 7):rounded(4):bg("warning"):no_shrink():mt(6)
		):child(b.text(open.problem):text_size(SIZE.small):color("warning"):wrap():min_w(0):grow(1))
	end
	return b.spacer()
end

-- The field of the edit of the issue `id`, `room` points across: nil where no edit of `target` is open,
-- or it has not begun (the words are drawn as they are).
function edit.field(id, target, room)
	local open = edit.of(id, target)
	if open == nil or open.status ~= "ready" then
		return nil
	end
	local t = u.theme
	local name = editor_name(id)
	-- A field that is not on the page yet is made now, empty: the words kept are put in it when it says
	-- it is ready.
	local fresh = b.layout.editor_version(name) == nil
	if E.drawn ~= id or fresh then
		E.drawn = id
		E.seen[name] = nil
		local kept = E.drafts[id]
		E.loading = (kept ~= nil and kept ~= "") and id or nil
	end
	local field = u:field(name)
	local ringed = u:keyboard() == field
	local words = words_for(id)
	return b.div()
		:id("edit-box")
		:row()
		:h(target == "description" and height(words, room, 120, 96) or height(words, room, 76, 44))
		:no_shrink()
		:rounded(t.radius.control)
		:bg(t.field)
		:border(1, ringed and t.ring or t.line)
		:motion({ seconds = t.motion, color = true })
		:child(b.editor(field, {
			wrap = true,
			gutter = false,
			padding = 8,
			line_height = 1.5,
			hint = target == "description" and "Add a description" or "Leave a comment",
			read_only = open.saving,
			-- Tab is the page's: it goes on to Save.
			keymap = { tab = false, ["shift+tab"] = false },
		}):grow(1):min_h(0):label(target == "description" and "Description" or "Comment"))
end

-- Save and Cancel, under the field of the edit of the issue `id` (and the files chosen for a
-- description), with why the last Save was not done after them.
function edit.actions(id)
	local open = E.open[id]
	local why = why_not(id)
	return b.div():row():items_start():gap(8):no_shrink():child(kit.button("edit-save", "Save", {
		primary = true,
		busy = open.saving,
		disabled = why ~= nil,
		suffix = commands.label("edit_save"),
		tip = why == nil and "Write it to Linear" or (why ~= "" and why or nil),
	})):child(kit.button("edit-cancel", "Cancel", {
		suffix = edit.cancelable() and "Esc" or nil,
		disabled = open.saving,
		tip = "Let go of the edit",
	})):child(said(id))
end

-- Why an edit of `target` of the issue `id` could not begin, said under its place, or nil.
function edit.failure(id, target)
	local failed = E.failed[id]
	if failed == nil or failed.target ~= target then
		return nil
	end
	return b.div():row():items_start():gap(8):no_shrink():mt(6):child(
		b.div():size(7, 7):rounded(4):bg("warning"):no_shrink():mt(6)
	):child(b.text(failed.words):text_size(SIZE.small):color("warning"):wrap():min_w(0):grow(1))
end

-- Edit description, beside Attach file under the description of the issue `id`: it stays where it is, and
-- is dim while an edit of the issue is open.
function edit.description_button(id)
	local open = E.open[id]
	local tip = "Edit the words of the description"
	if open ~= nil then
		tip = open.target == "description" and "It is being edited" or "Save or cancel the edit of the comment first"
	end
	return kit.button("edit-description", "Edit description", {
		suffix = commands.label("edit_description"),
		disabled = open ~= nil,
		busy = open ~= nil and open.target == "description" and open.status == "loading",
		tip = tip,
	})
end

-- Edit comment, at the end of the line of a comment of the person's (`one`) on the issue `id`. Its key is
-- written on the last of them, which the key edits.
function edit.comment_button(id, one, last)
	local open = E.open[id]
	local tip = "Edit the words of your comment"
	if open ~= nil then
		tip = open.target == one.id and "It is being edited" or "Save or cancel the other edit first"
	end
	return kit.button("edit-comment:" .. one.id, "Edit comment", {
		small = true,
		suffix = last and commands.label("edit_comment") or nil,
		disabled = open ~= nil,
		busy = open ~= nil and open.target == one.id and open.status == "loading",
		tip = tip,
	})
end

-- A press of one of the buttons of this part. The reply, or nil where it was not this part's.
function edit.press(id)
	local issue = act.panel_id()
	if id == "edit-description" then
		return edit.begin(issue, "description")
	elseif string.sub(id, 1, 13) == "edit-comment:" then
		return edit.begin(issue, string.sub(id, 14))
	elseif id == "edit-save" then
		return edit.save(issue)
	elseif id == "edit-cancel" then
		return edit.ask_cancel(issue)
	end
	return nil
end

act.edit_describing = function(id)
	return edit.of(id, "description") ~= nil
end
-- The keyboard goes back to the field of the edit of the description of the issue `id` (files were just
-- added to it), where it is on the page. The reply, or nil.
act.edit_focus = function(id)
	if ready(id) and E.open[id].target == "description" and act.panel_id() == id then
		return edit.focus(id)
	end
	return nil
end

return edit
