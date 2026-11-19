-- The files the person adds: to a comment, to the issue that is being made, and to the end of the
-- description of the issue that is open. They come as files the terminal gives the page (pasted into a
-- field, dropped on a place, or chosen with Attach file), each a handle and a name: the page keeps the
-- handles of what was chosen, draws each small with Remove, and passes the handles on to the program
-- with what it sends (`comment`, `create`, `describe`). The page reads no file and knows no address;
-- a file of any kind is taken, up to 100 MiB (a folder, or one that is over that, is turned away here
-- with words), and the program holds the same to the bytes. A picture is drawn small; any other file is
-- a card with the icon of its kind.

local kit = require("../lib/kit")
local commands = require("./commands")
local kinds = require("./kinds")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local attach = {}

-- The most files one send carries, and the most bytes of one.
local MOST = 10
local BYTES = 100 * 1024 * 1024
local MEBIBYTES = BYTES // (1024 * 1024)

-- The width of the card of a file in a tray, points, and the pane width under which a card has a row.
local CARD = 240
local NARROW = 600

-- What the person is told of a file that is turned away.
local TOO_LARGE = "Files are up to " .. MEBIBYTES .. " MiB"
local FOLDER = "A folder cannot be attached"

-- What the page holds: the files chosen for each place, by key (`comment:<issue>`, `new`,
-- `describe:<issue>`), each `{ handle, name, size, type }`; the place the Open panel was asked for; the
-- places that are being sent and why one was not; and what each Remove of the frame stands for (made as
-- the frame is drawn).
local A = { trays = {}, picking = nil, sending = {}, problems = {}, removers = {}, count = 0, reveal = nil }
attach.state = A

-- Whether a file the terminal gave can be sent: a file (not a folder), not over 100 MiB. The program
-- holds it to the bytes. Nil, and why (`folder` or `size`), where it cannot.
local function check(file)
	if file.kind == "folder" or file.handle == nil then
		return nil, "folder"
	end
	if file.size ~= nil and file.size > BYTES then
		return nil, "size"
	end
	return true
end

-- The name a file is told by: the person's, or `image.png` where a pasted picture has none, else `file`.
local function name_of(file)
	local name = file.name
	if name == nil or name == "" then
		local made = kinds.extension_of_type(file.type)
		name = made ~= nil and ("image." .. made) or "file"
	end
	return name
end

function attach.count(key)
	return A.trays[key] ~= nil and #A.trays[key] or 0
end

-- What goes to the program with a send: `{ { handle =, name = } }`, or nil where there is none.
function attach.params(key)
	local tray = A.trays[key]
	if tray == nil or #tray == 0 then
		return nil
	end
	local out = {}
	for _, item in tray do
		out[#out + 1] = { handle = item.handle, name = item.name }
	end
	return out
end

function attach.clear(key)
	A.trays[key] = nil
	A.problems[key] = nil
end

-- Lets go of the files of `key` that were sent (`sent` is what `params` gave, or nil where nothing was
-- sent) and of no other: a file the person added while the others were on their way waits in the tray
-- for the next send.
function attach.release(key, sent)
	local tray = A.trays[key]
	A.problems[key] = nil
	if tray == nil then
		return
	end
	local gone = {}
	for _, file in sent or {} do
		gone[file.handle] = true
	end
	local kept = {}
	for _, item in tray do
		if not gone[item.handle] then
			kept[#kept + 1] = item
		end
	end
	A.trays[key] = #kept > 0 and kept or nil
end

-- Whether the files of `key` are on their way to Linear.
function attach.sending(key)
	return A.sending[key] == true
end

-- Gives the tray of `key` the files that can be sent; the person is told of those that cannot.
function attach.add(key, files)
	local tray = A.trays[key]
	if tray == nil then
		tray = {}
		A.trays[key] = tray
	end
	local turned, full = nil, false
	for _, file in files do
		local fine, why = check(file)
		if not fine then
			turned = turned or why
		elseif #tray >= MOST then
			full = true
		else
			tray[#tray + 1] = { handle = file.handle, name = name_of(file), size = file.size or 0, type = file.type }
		end
	end
	if turned ~= nil then
		kit.toast(turned == "folder" and FOLDER or TOO_LARGE, "warn")
	end
	if full then
		kit.toast("Up to " .. MOST .. " files at a time", "warn")
	end
	A.problems[key] = nil
	-- The tray is laid out below where the person is: the room is scrolled to show it (`settle`).
	if #tray > 0 then
		if string.sub(key, 1, 9) == "describe:" then
			A.reveal = { zone = "describe-zone", mark = "describe-add" }
		elseif string.sub(key, 1, 8) == "comment:" then
			A.reveal = { zone = "comment-box", mark = "comment-send" }
		end
	end
end

-- Once the tray is laid out, the room the issue is read in is scrolled as far as the foot of the place
-- the files were put in needs, and no further. Whether the page needs another frame.
function attach.settle()
	local reveal = A.reveal
	if reveal == nil then
		return false
	end
	local zone, mark = b.layout.bounds(reveal.zone), b.layout.bounds(reveal.mark)
	if zone == nil then
		A.reveal = nil
		return false
	end
	if mark == nil then
		-- (The next frame lays it out.)
		return false
	end
	A.reveal = nil
	local view = act.panel_view()
	if view == nil then
		return false
	end
	local over = zone.y + zone.h - (view.y + view.h)
	if over > 0 then
		act.panel_scroll((act.panel_scroll() or 0) + over + 16)
		return true
	end
	return false
end

-- A frame is drawn: the Removes are numbered again.
function attach.begin()
	A.count = 0
	A.removers = {}
end

-- --- Drawing -----------------------------------------------------------------------------------------

-- Attach file, for the place named `kind` (`comment`, `new` or `describe`). The key is written once: in
-- the button of the place the key is for now (`act.attach_kind`, the page's).
function attach.button(kind, locked)
	local keyed = act.attach_kind ~= nil and act.attach_kind() == kind
	return kit.button("attach-file:" .. kind, "Attach file", {
		suffix = keyed and commands.label("attach_file") or nil,
		disabled = locked,
		tip = locked and "Wait until the files are sent" or "Choose files of any kind, up to " .. MEBIBYTES .. " MiB each",
	})
end

-- What the words say while the files of `key` are sent.
function attach.uploading(key)
	local count = attach.count(key)
	return count == 1 and "Uploading 1 file" or ("Uploading " .. count .. " files")
end

-- The files chosen for `key`, each with its picture (small) or the icon of its kind, its name, its size and
-- Remove; nil where there are none. They are all of one width (the kit shortens a name that does not fit,
-- and the tip says it whole), and wrap in the room they have; in a narrow pane each has a row.
function attach.tray(key, locked)
	local tray = A.trays[key]
	if tray == nil or #tray == 0 then
		return nil
	end
	local t = u.theme
	local row = b.div():row():wrap():gap(8):no_shrink()
	for n, item in tray do
		A.count += 1
		local id = "file-remove:" .. A.count
		A.removers[id] = { key = key, n = n }
		local card = b.div():id("tray-file:" .. A.count):row():items_center():gap(8):p(6):rounded(t.radius.control):border(1, t.line):no_shrink():tip(item.name)
		if S.width < NARROW then
			card:w_full()
		else
			card:w(CARD)
		end
		local thumb = b.div():size(44, 34):no_shrink():rounded(4):clip():bg(b.alpha("foreground", 0.06))
		if kinds.drawn_by_type(item.type, item.name) then
			thumb:child(b.image("file:" .. item.handle):size(44, 34):fit("cover"))
		else
			thumb:items_center():justify_center():child(kit.icon(kinds.icon(item.name, item.type), 20, "dim"))
		end
		card:child(thumb)
		card:child(
			b.div():col():gap(1):min_w(0):grow(1):child(
				b.text(item.name):text_size(SIZE.small):truncate()
			):child(b.text(kinds.size_words(item.size)):text_size(SIZE.tiny):color("dim"))
		)
		card:child(kit.button(id, "Remove", {
			small = true,
			disabled = locked,
			tip = locked and "Wait until the files are sent" or "Take " .. item.name .. " out",
		}))
		row:child(card)
	end
	return row
end

-- The words that say why the files of `key` were not sent, or nil.
function attach.problem(key)
	return A.problems[key]
end

-- --- Description --------------------------------------------------------------------------------------

-- The issue `id`'s own key.
local function described(id)
	return "describe:" .. id
end

-- Whether files are waiting to be added to the description of the issue that is open.
function attach.describable()
	local id = act.panel_id()
	return id ~= nil and attach.count(described(id)) > 0 and not A.sending[described(id)]
end

-- Sends the files chosen for the description of the issue open, and has them added at its end. The
-- reply.
function attach.describe()
	local id = act.panel_id()
	if id == nil then
		return true
	end
	local key = described(id)
	local files = attach.params(key)
	if files == nil or A.sending[key] then
		return true
	end
	A.sending[key], A.problems[key] = true, nil
	kit.call("describe", { id = id, files = files }, function(_, err)
		A.sending[key] = nil
		if err then
			A.problems[key] = err.message or err.code
			return true
		end
		attach.release(key, files)
		-- The issue is asked for again, with the files in its description.
		if not act.panel_reask(id, function() end) then
			act.panel_forget(id)
		end
		return true
	end)
	return true
end

-- Whether Esc lets go of files chosen for the description of the issue open (there are some, and they are
-- not being sent): then it is Cancel's, written on Cancel.
function attach.cancelable()
	local id = act.panel_id()
	return id ~= nil and attach.count(described(id)) > 0 and not A.sending[described(id)]
end

-- Esc: what was chosen for the description of the issue open is let go. Whether there was any.
function attach.cancel()
	if not attach.cancelable() then
		return false
	end
	attach.clear(described(act.panel_id()))
	return true
end

-- Under the description: Attach file, and, where files were chosen, them with Add to description and
-- Cancel. The place has an id, which is where a drop on the description is the description's.
function attach.zone(id, text)
	local key = described(id)
	local sending = A.sending[key] == true
	local zone = b.div():id("describe-zone"):col():gap(8):no_shrink()
	zone:child(text)
	local row = b.div():row():items_center():gap(8):mt(10):no_shrink():child(attach.button("describe", sending))
	zone:child(row)
	local tray = attach.tray(key, sending)
	if tray ~= nil then
		zone:child(tray)
		local said = b.spacer()
		if sending then
			said = b.text(attach.uploading(key)):text_size(SIZE.small):color("dim"):grow(1)
		end
		zone:child(
			b.div():row():items_center():gap(8):no_shrink():child(kit.button("describe-add", "Add to description", {
				primary = true,
				busy = sending,
				disabled = sending,
				suffix = commands.label("describe"),
				tip = "Put the files at the end of the description, in Linear",
			})):child(kit.button("describe-cancel", "Cancel", { suffix = "Esc", disabled = sending })):child(said)
		)
	end
	local problem = A.problems[key]
	if problem ~= nil then
		zone:child(
			b.div():row():items_start():gap(8):no_shrink():child(
				b.div():size(7, 7):rounded(4):bg("warning"):no_shrink():mt(6)
			):child(b.text(problem):text_size(SIZE.small):color("warning"):wrap():min_w(0):grow(1))
		)
	end
	return zone
end

-- --- What the person does ------------------------------------------------------------------------------

-- The Open panel is asked for, for the files of `key`. The reply (a panel is shown only for a reply
-- to something the person did: a press, a key, a command).
function attach.choose(key)
	A.picking = key
	return { pick = { multiple = true } }
end

-- The terminal gave the page files. `places` says where they go: `paste()` the key of the place a paste
-- goes to, `drop(x, y)` the key of the one a drop at that point is on (nil: none). The reply.
function attach.files(event, places)
	local picked = A.picking
	if event.source == "pick" then
		A.picking = nil
	end
	local files = event.files
	if files == nil or #files == 0 then
		return true
	end
	local key = nil
	if event.source == "pick" then
		key = picked
	elseif event.source == "paste" then
		key = places.paste()
	elseif event.source == "drop" then
		key = places.drop(event.x, event.y)
		if key == nil then
			kit.toast("Drop files on the comment, on the description or on the new issue")
		end
	end
	if key ~= nil then
		attach.add(key, files)
		-- The field of the comment has the keyboard again where the files are its own: the words are
		-- what the person goes on with.
		local issue = string.match(key, "^comment:(.+)$")
		if issue ~= nil and act.comment_focus ~= nil then
			return act.comment_focus(issue)
		end
	end
	return true
end

-- A press of one of the buttons of this part. `key_of(kind)` says the key of the place a kind of Attach
-- file is of now. The reply, or nil where it was not this part's.
function attach.press(id, key_of)
	if string.sub(id, 1, 12) == "attach-file:" then
		local key = key_of(string.sub(id, 13))
		if key == nil then
			return true
		end
		return attach.choose(key)
	elseif string.sub(id, 1, 12) == "file-remove:" then
		local one = A.removers[id]
		local tray = one ~= nil and A.trays[one.key] or nil
		if tray ~= nil and not A.sending[one.key] then
			table.remove(tray, one.n)
			if #tray == 0 then
				A.trays[one.key] = nil
			end
		end
		local issue = one ~= nil and string.match(one.key, "^comment:(.+)$") or nil
		if issue ~= nil and act.comment_focus ~= nil then
			return act.comment_focus(issue)
		end
		return true
	elseif id == "describe-add" then
		return attach.describe()
	elseif id == "describe-cancel" then
		attach.cancel()
		return true
	end
	return nil
end

return attach
