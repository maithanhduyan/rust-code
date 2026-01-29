# Tiến Lên Miền Nam (Southern Thirteen) - Game Research & Architecture

## 1. Tổng Quan (Overview)

### 1.1 Giới Thiệu Game
**Tiến Lên Miền Nam** (còn gọi là "Thirteen" hoặc "Tien Len") là game bài phổ biến tại Việt Nam. Đây là game thuộc thể loại "shedding-type" - mục tiêu là đánh hết bài trước đối thủ.

### 1.2 Thông Tin Cơ Bản
| Thuộc tính | Giá trị |
|-----------|---------|
| Số người chơi | 4 players / phòng |
| Bộ bài | 52 lá (Standard deck) |
| Số lá / người | 13 lá |
| Dealer | Host / AI / Algorithm |
| Thời gian / ván | ~5-10 phút |

---

## 2. Luật Chơi Chi Tiết

### 2.1 Thứ Hạng Bài

#### Rank (cao → thấp):
```
2 > A > K > Q > J > 10 > 9 > 8 > 7 > 6 > 5 > 4 > 3
```

#### Suit (cao → thấp):
```
♥ Hearts > ♦ Diamonds > ♣ Clubs > ♠ Spades
```

#### Kết hợp:
- **Cao nhất**: 2♥ (Heo đỏ)
- **Thấp nhất**: 3♠

### 2.2 Các Loại Bài Đánh Được

| Loại | Mô tả | Ví dụ |
|------|-------|-------|
| **Single** | 1 lá | K♥ |
| **Pair** | Đôi (2 lá cùng rank) | 9♦ 9♣ |
| **Triple** | Ba lá cùng rank | Q♥ Q♦ Q♣ |
| **Quartet** | Tứ quý (4 lá cùng rank) | A♥ A♦ A♣ A♠ |
| **Sequence** | Sảnh (≥3 lá liên tiếp) | 9♣ 10♦ J♣ |
| **Double Sequence** | Sảnh đôi (≥3 đôi liên tiếp) | 5♣ 5♠ 6♥ 6♦ 7♣ 7♦ |

### 2.3 Luật Đánh

1. **Ván đầu tiên**: Người có 3♠ đánh trước
2. **Các ván sau**: Người thắng ván trước đánh trước
3. **Theo bài**: Phải đánh cùng loại và cao hơn bài trước
4. **Pass**: Nếu không muốn/có bài để theo, phải pass. Sau khi pass không được đánh tiếp round đó
5. **Thắng round**: Khi tất cả pass, người đánh cuối thắng round và dẫn bài mới

### 2.4 Chặt Heo (Bomb Rules)

Các combo đặc biệt có thể "chặt" (đánh thắng) bài 2:

| Bài 2 | Có thể bị chặt bởi |
|-------|-------------------|
| Single 2 | Tứ quý HOẶC Sảnh đôi 3+ đôi |
| Pair 2 | Tứ quý HOẶC Sảnh đôi 4+ đôi |
| Triple 2 | Sảnh đôi 5+ đôi |

### 2.5 Kết Thúc Game

- Người hết bài đầu tiên → **Nhất**
- Người còn bài cuối cùng → **Bét** (thua)

---

## 3. Kiến Trúc Hệ Thống (Based on snake-rs)

### 3.1 So Sánh với Snake-RS

| Thành phần | Snake-RS | Thirteen Card |
|------------|----------|---------------|
| Entity chính | `Snake` | `Player` + `Hand` |
| State container | `snakes: DashMap<Uuid, Snake>` | `rooms: DashMap<Uuid, Room>` |
| Game loop | 100ms tick | Turn-based (event-driven) |
| Broadcast | Real-time positions | Game state changes |
| Food | Random spawn | Card dealing |

### 3.2 Cấu Trúc Thư Mục Đề Xuất

```
thirteen-rs/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── state.rs              # AppState với rooms
│   ├── broadcast.rs          # Broadcaster trait (giữ nguyên từ snake-rs)
│   ├── protocol.rs           # Messages cho card game
│   ├── rate_limiter.rs       # Anti-cheat (giữ nguyên)
│   ├── event_logger.rs       # Event logging (mở rộng)
│   │
│   ├── room/
│   │   ├── mod.rs
│   │   ├── room.rs           # Room entity
│   │   ├── room_manager.rs   # Room creation/join/leave
│   │   └── turn_manager.rs   # Turn handling
│   │
│   ├── game/
│   │   ├── mod.rs
│   │   ├── card.rs           # Card struct
│   │   ├── deck.rs           # Deck + Shuffle
│   │   ├── hand.rs           # Player hand
│   │   ├── combo.rs          # Card combinations
│   │   ├── validator.rs      # Move validation
│   │   └── dealer.rs         # AI/Algorithm dealer
│   │
│   └── ws/
│       ├── mod.rs
│       └── handler.rs        # WebSocket handler
│
└── static/
    ├── index.html
    ├── game.js
    └── style.css
```

---

## 4. Core Data Structures

### 4.1 Card (Lá bài)

```rust
// src/game/card.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Suit {
    Spades = 0,   // ♠ Lowest
    Clubs = 1,    // ♣
    Diamonds = 2, // ♦
    Hearts = 3,   // ♥ Highest
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rank {
    Three = 0,
    Four = 1,
    Five = 2,
    Six = 3,
    Seven = 4,
    Eight = 5,
    Nine = 6,
    Ten = 7,
    Jack = 8,
    Queen = 9,
    King = 10,
    Ace = 11,
    Two = 12, // Highest
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl Card {
    /// Compare two cards (higher is better)
    pub fn compare(&self, other: &Card) -> std::cmp::Ordering {
        match self.rank.cmp(&other.rank) {
            std::cmp::Ordering::Equal => self.suit.cmp(&other.suit),
            ord => ord,
        }
    }

    /// Check if this is 3♠ (lowest card)
    pub fn is_three_spades(&self) -> bool {
        self.rank == Rank::Three && self.suit == Suit::Spades
    }
}
```

### 4.2 Combo (Các loại bài đánh)

```rust
// src/game/combo.rs

#[derive(Debug, Clone, PartialEq)]
pub enum ComboType {
    Single,
    Pair,
    Triple,
    Quartet,
    Sequence(u8),       // Length of sequence
    DoubleSequence(u8), // Number of pairs
}

#[derive(Debug, Clone)]
pub struct Combo {
    pub combo_type: ComboType,
    pub cards: Vec<Card>,
    pub highest_card: Card,
}

impl Combo {
    /// Validate if cards form a valid combo
    pub fn from_cards(cards: Vec<Card>) -> Option<Self> {
        // Implementation: detect combo type
        todo!()
    }

    /// Check if this combo can beat another
    pub fn can_beat(&self, other: &Combo) -> bool {
        // Same type, higher card
        // OR bomb rules
        todo!()
    }

    /// Check if this combo can "chặt heo" (beat 2s)
    pub fn can_bomb(&self, twos: &Combo) -> bool {
        match (&self.combo_type, &twos.combo_type) {
            // Single 2 beaten by quartet or 3+ pair sequence
            (ComboType::Quartet, ComboType::Single) => true,
            (ComboType::DoubleSequence(n), ComboType::Single) if *n >= 3 => true,

            // Pair 2s beaten by quartet or 4+ pair sequence
            (ComboType::Quartet, ComboType::Pair) => true,
            (ComboType::DoubleSequence(n), ComboType::Pair) if *n >= 4 => true,

            // Triple 2s beaten by 5+ pair sequence
            (ComboType::DoubleSequence(n), ComboType::Triple) if *n >= 5 => true,

            _ => false,
        }
    }
}
```

### 4.3 Player State

```rust
// src/game/hand.rs

#[derive(Debug, Clone)]
pub struct Hand {
    pub cards: Vec<Card>,
}

impl Hand {
    pub fn new() -> Self {
        Self { cards: Vec::with_capacity(13) }
    }

    pub fn add_card(&mut self, card: Card) {
        self.cards.push(card);
        self.sort();
    }

    pub fn remove_cards(&mut self, cards: &[Card]) -> bool {
        // Remove cards from hand
        todo!()
    }

    pub fn has_three_spades(&self) -> bool {
        self.cards.iter().any(|c| c.is_three_spades())
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    fn sort(&mut self) {
        self.cards.sort_by(|a, b| a.compare(b));
    }
}
```

### 4.4 Room & Player

```rust
// src/room/room.rs

use uuid::Uuid;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum RoomStatus {
    Waiting,    // Waiting for players
    Playing,    // Game in progress
    Finished,   // Game ended
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerStatus {
    Playing,    // Still has cards
    Finished,   // Đã hết bài (with rank: 1st, 2nd, 3rd)
    Passed,     // Passed this round
}

#[derive(Debug, Clone)]
pub struct Player {
    pub id: Uuid,
    pub seat: u8,           // 0-3
    pub hand: Hand,
    pub status: PlayerStatus,
    pub finish_rank: Option<u8>, // 1 = Nhất, 4 = Bét
}

#[derive(Debug)]
pub struct Room {
    pub id: Uuid,
    pub code: String,       // 6-char room code
    pub players: HashMap<Uuid, Player>,
    pub status: RoomStatus,
    pub current_turn: u8,   // Seat index 0-3
    pub current_combo: Option<Combo>,
    pub passed_players: Vec<u8>,
    pub finish_order: Vec<Uuid>,
}

impl Room {
    pub const MAX_PLAYERS: usize = 4;

    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            code: Self::generate_code(),
            players: HashMap::new(),
            status: RoomStatus::Waiting,
            current_turn: 0,
            current_combo: None,
            passed_players: Vec::new(),
            finish_order: Vec::new(),
        }
    }

    fn generate_code() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..6).map(|_| rng.gen_range(b'A'..=b'Z') as char).collect()
    }

    pub fn is_full(&self) -> bool {
        self.players.len() >= Self::MAX_PLAYERS
    }

    pub fn can_start(&self) -> bool {
        self.players.len() == Self::MAX_PLAYERS && self.status == RoomStatus::Waiting
    }
}
```

---

## 5. Protocol Messages

### 5.1 Client → Server

```rust
// src/protocol.rs

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    // Room management
    CreateRoom,
    JoinRoom { code: String },
    LeaveRoom,
    StartGame,

    // Gameplay
    PlayCards { cards: Vec<CardData> },
    Pass,

    // Misc
    Ping,
}

#[derive(Debug, Deserialize)]
pub struct CardData {
    pub rank: u8,  // 0-12 (3 to 2)
    pub suit: u8,  // 0-3 (♠♣♦♥)
}
```

### 5.2 Server → Client

```rust
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    // Connection
    Connected { player_id: String },

    // Room events
    RoomCreated { code: String },
    RoomJoined {
        code: String,
        players: Vec<PlayerData>,
        your_seat: u8,
    },
    PlayerJoined { player: PlayerData },
    PlayerLeft { seat: u8 },

    // Game start
    GameStarted {
        your_cards: Vec<CardData>,
        first_turn: u8,
    },

    // Gameplay
    CardsPlayed {
        seat: u8,
        cards: Vec<CardData>,
        cards_remaining: u8,
    },
    PlayerPassed { seat: u8 },
    TurnChanged { seat: u8 },
    RoundWon { seat: u8 },

    // Game end
    PlayerFinished { seat: u8, rank: u8 },
    GameEnded { rankings: Vec<RankingData> },

    // Errors
    Error { message: String },

    // Pong
    Pong,
}

#[derive(Debug, Serialize)]
pub struct PlayerData {
    pub id: String,
    pub seat: u8,
    pub card_count: u8,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct RankingData {
    pub seat: u8,
    pub rank: u8,  // 1-4
}
```

---

## 6. State Management

### 6.1 AppState (Tương tự snake-rs)

```rust
// src/state.rs

use dashmap::DashMap;
use uuid::Uuid;
use std::sync::Arc;

pub struct AppState {
    /// All active rooms
    pub rooms: DashMap<String, Arc<RwLock<Room>>>,  // code -> Room

    /// Player to room mapping
    pub player_rooms: DashMap<Uuid, String>,  // player_id -> room_code

    /// Broadcaster for messages
    pub broadcaster: Arc<dyn Broadcaster>,

    /// Rate limiter (reuse từ snake-rs)
    pub rate_limiter: RateLimiter,

    /// Event logger (reuse từ snake-rs)
    pub event_logger: Arc<EventLogger>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            rooms: DashMap::new(),
            player_rooms: DashMap::new(),
            broadcaster: Arc::new(InMemoryBroadcaster::new()),
            rate_limiter: RateLimiter::new(),
            event_logger: Arc::new(EventLogger::new()),
        }
    }

    pub fn create_room(&self) -> String {
        let room = Room::new();
        let code = room.code.clone();
        self.rooms.insert(code.clone(), Arc::new(RwLock::new(room)));
        code
    }

    pub fn find_room(&self, code: &str) -> Option<Arc<RwLock<Room>>> {
        self.rooms.get(code).map(|r| r.clone())
    }
}
```

### 6.2 Broadcast Strategy

```rust
/// Room-scoped broadcasting (khác với snake-rs broadcast toàn server)
pub struct RoomBroadcaster {
    /// Room code -> Broadcast channel
    channels: DashMap<String, broadcast::Sender<ServerMessage>>,
}

impl RoomBroadcaster {
    pub fn subscribe(&self, room_code: &str) -> broadcast::Receiver<ServerMessage> {
        // Get or create channel for room
        todo!()
    }

    pub fn send_to_room(&self, room_code: &str, msg: ServerMessage) {
        if let Some(tx) = self.channels.get(room_code) {
            let _ = tx.send(msg);
        }
    }

    /// Send to specific player in room
    pub fn send_to_player(&self, player_id: &Uuid, msg: ServerMessage) {
        // Use per-player channel
        todo!()
    }
}
```

---

## 7. Game Logic

### 7.1 Dealer (Chia bài)

```rust
// src/game/dealer.rs

use rand::seq::SliceRandom;
use rand::thread_rng;

pub struct Dealer;

impl Dealer {
    /// Create and shuffle a new deck
    pub fn create_deck() -> Vec<Card> {
        let mut deck = Vec::with_capacity(52);

        for suit in [Suit::Spades, Suit::Clubs, Suit::Diamonds, Suit::Hearts] {
            for rank in [
                Rank::Three, Rank::Four, Rank::Five, Rank::Six,
                Rank::Seven, Rank::Eight, Rank::Nine, Rank::Ten,
                Rank::Jack, Rank::Queen, Rank::King, Rank::Ace, Rank::Two,
            ] {
                deck.push(Card { rank, suit });
            }
        }

        deck.shuffle(&mut thread_rng());
        deck
    }

    /// Deal cards to 4 players
    pub fn deal(players: &mut [Player; 4]) {
        let mut deck = Self::create_deck();

        // Each player gets 13 cards
        for (i, player) in players.iter_mut().enumerate() {
            player.hand = Hand::new();
            for j in 0..13 {
                player.hand.add_card(deck[i * 13 + j]);
            }
        }
    }

    /// Find who has 3♠ (first turn)
    pub fn find_first_player(players: &[Player]) -> u8 {
        players
            .iter()
            .enumerate()
            .find(|(_, p)| p.hand.has_three_spades())
            .map(|(i, _)| i as u8)
            .unwrap_or(0)
    }
}
```

### 7.2 Turn Manager

```rust
// src/room/turn_manager.rs

impl Room {
    /// Process a play move
    pub fn play_cards(&mut self, player_id: &Uuid, cards: Vec<Card>) -> Result<(), GameError> {
        let player = self.players.get_mut(player_id)
            .ok_or(GameError::PlayerNotFound)?;

        // Check if it's this player's turn
        if player.seat != self.current_turn {
            return Err(GameError::NotYourTurn);
        }

        // Validate the combo
        let combo = Combo::from_cards(cards.clone())
            .ok_or(GameError::InvalidCombo)?;

        // First play of the game must include 3♠
        if self.is_first_play() && !cards.iter().any(|c| c.is_three_spades()) {
            return Err(GameError::MustPlayThreeSpades);
        }

        // Check if combo beats current
        if let Some(ref current) = self.current_combo {
            if !combo.can_beat(current) && !combo.can_bomb(current) {
                return Err(GameError::ComboCantBeat);
            }
        }

        // Remove cards from player's hand
        player.hand.remove_cards(&cards);

        // Update game state
        self.current_combo = Some(combo);
        self.passed_players.clear();

        // Check if player finished
        if player.hand.is_empty() {
            self.finish_order.push(*player_id);
            player.finish_rank = Some(self.finish_order.len() as u8);
            player.status = PlayerStatus::Finished;
        }

        self.advance_turn();
        Ok(())
    }

    /// Process a pass
    pub fn pass(&mut self, player_id: &Uuid) -> Result<(), GameError> {
        let player = self.players.get_mut(player_id)
            .ok_or(GameError::PlayerNotFound)?;

        if player.seat != self.current_turn {
            return Err(GameError::NotYourTurn);
        }

        // Can't pass if leading (no current combo)
        if self.current_combo.is_none() {
            return Err(GameError::CantPassWhenLeading);
        }

        self.passed_players.push(player.seat);
        player.status = PlayerStatus::Passed;

        self.advance_turn();

        // Check if round is won
        if self.is_round_won() {
            self.start_new_round();
        }

        Ok(())
    }

    fn advance_turn(&mut self) {
        loop {
            self.current_turn = (self.current_turn + 1) % 4;

            // Skip finished and passed players
            let player = self.players.values()
                .find(|p| p.seat == self.current_turn);

            if let Some(p) = player {
                if p.status == PlayerStatus::Playing {
                    break;
                }
            }
        }
    }

    fn is_round_won(&self) -> bool {
        // All other players passed
        let active_count = self.players.values()
            .filter(|p| p.status == PlayerStatus::Playing)
            .count();

        self.passed_players.len() >= active_count - 1
    }

    fn start_new_round(&mut self) {
        self.current_combo = None;
        self.passed_players.clear();

        // Reset passed status
        for player in self.players.values_mut() {
            if player.status == PlayerStatus::Passed {
                player.status = PlayerStatus::Playing;
            }
        }
    }
}
```

---

## 8. WebSocket Handler

```rust
// src/ws/handler.rs (adapted from snake-rs)

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let player_id = Uuid::new_v4();
    let (mut sender, mut receiver) = socket.split();

    // Send connected message
    let _ = sender.send(Message::Text(
        ServerMessage::Connected { player_id: player_id.to_string() }.to_json().into()
    )).await;

    // Register with rate limiter
    state.rate_limiter.add_player(player_id);

    // Track which room this player is in
    let mut current_room: Option<String> = None;
    let mut room_rx: Option<broadcast::Receiver<ServerMessage>> = None;

    loop {
        tokio::select! {
            // Receive from client
            Some(Ok(msg)) = receiver.next() => {
                if let Message::Text(text) = msg {
                    if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                        // Rate limit check
                        let (allowed, kick) = state.rate_limiter.check_command(&player_id);
                        if kick {
                            break;
                        }
                        if !allowed {
                            continue;
                        }

                        // Process message
                        match client_msg {
                            ClientMessage::CreateRoom => {
                                let code = state.create_room();
                                current_room = Some(code.clone());
                                // Subscribe to room broadcasts
                                // Send RoomCreated
                            }
                            ClientMessage::JoinRoom { code } => {
                                // Join existing room
                                // Subscribe to room broadcasts
                                // Send RoomJoined
                            }
                            ClientMessage::PlayCards { cards } => {
                                // Validate and play
                            }
                            ClientMessage::Pass => {
                                // Process pass
                            }
                            // ... other handlers
                        }
                    }
                }
            }

            // Receive room broadcasts
            Some(msg) = async {
                if let Some(ref mut rx) = room_rx {
                    rx.recv().await.ok()
                } else {
                    None
                }
            } => {
                let _ = sender.send(Message::Text(msg.to_json().into())).await;
            }
        }
    }

    // Cleanup on disconnect
    if let Some(code) = current_room {
        // Remove player from room
        // Broadcast PlayerLeft
    }
    state.rate_limiter.remove_player(&player_id);
}
```

---

## 9. Anti-Cheat (Reuse từ snake-rs)

### 9.1 Rate Limiter
Giữ nguyên implementation từ snake-rs với config:

```rust
// src/config.rs

// Anti-cheat settings
pub const MAX_COMMANDS_PER_SECOND: usize = 10; // Lower than snake (card game slower)
pub const RATE_LIMIT_WINDOW_MS: u64 = 1000;
pub const MAX_RATE_VIOLATIONS: u32 = 3;
```

### 9.2 Game Event Logger

Mở rộng event types:

```rust
// src/event_logger.rs

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum GameEvent {
    // Room events
    RoomCreated { room_code: String, host_id: String },
    PlayerJoinedRoom { room_code: String, player_id: String, seat: u8 },
    PlayerLeftRoom { room_code: String, player_id: String },

    // Game events
    GameStarted { room_code: String },
    CardsDealt { room_code: String, player_id: String, cards: Vec<String> },
    CardsPlayed { room_code: String, player_id: String, cards: Vec<String> },
    PlayerPassed { room_code: String, player_id: String },
    RoundWon { room_code: String, winner_seat: u8 },
    PlayerFinished { room_code: String, player_id: String, rank: u8 },
    GameEnded { room_code: String, rankings: Vec<u8> },

    // Anti-cheat
    RateLimitViolation { player_id: String, room_code: String },
    PlayerKicked { player_id: String, reason: String },
    SuspiciousPlay { player_id: String, details: String },
}
```

---

## 10. Frontend Considerations

### 10.1 UI Components

```
┌─────────────────────────────────────────────────┐
│                 OPPONENT (TOP)                   │
│              [🂠] [🂠] [🂠] [🂠] [🂠]              │
│                                                  │
│    LEFT                           RIGHT          │
│   [🂠][🂠]                       [🂠][🂠]         │
│   [🂠][🂠]                       [🂠][🂠]         │
│   [🂠][🂠]                       [🂠]             │
│                                                  │
│              ┌──── TABLE ────┐                   │
│              │  [K♥] [K♦]    │                   │
│              └───────────────┘                   │
│                                                  │
│               YOUR CARDS (BOTTOM)                │
│   [3♠][4♦][5♥][7♣][8♦][9♥][10♠][J♦][Q♣][K♥]    │
│                                                  │
│      [  PLAY  ]    [  PASS  ]    [  SORT  ]     │
└─────────────────────────────────────────────────┘
```

### 10.2 Card Selection Logic (JavaScript)

```javascript
class CardGame {
    constructor() {
        this.ws = null;
        this.myCards = [];
        this.selectedCards = [];
        this.currentTurn = null;
        this.mySeat = null;
    }

    connect() {
        this.ws = new WebSocket(`ws://${location.host}/ws`);
        this.ws.onmessage = (e) => this.handleMessage(JSON.parse(e.data));
    }

    handleMessage(msg) {
        switch (msg.type) {
            case 'game_started':
                this.myCards = msg.your_cards;
                this.currentTurn = msg.first_turn;
                this.renderCards();
                break;
            case 'cards_played':
                this.showPlayedCards(msg.seat, msg.cards);
                this.updateCardCount(msg.seat, msg.cards_remaining);
                break;
            case 'turn_changed':
                this.currentTurn = msg.seat;
                this.updateTurnIndicator();
                break;
            // ... other cases
        }
    }

    toggleCard(cardIndex) {
        // Toggle selection
        if (this.selectedCards.includes(cardIndex)) {
            this.selectedCards = this.selectedCards.filter(i => i !== cardIndex);
        } else {
            this.selectedCards.push(cardIndex);
        }
        this.renderCards();
    }

    play() {
        if (this.currentTurn !== this.mySeat) return;

        const cards = this.selectedCards.map(i => this.myCards[i]);
        this.ws.send(JSON.stringify({
            type: 'play_cards',
            cards: cards
        }));
        this.selectedCards = [];
    }

    pass() {
        if (this.currentTurn !== this.mySeat) return;
        this.ws.send(JSON.stringify({ type: 'pass' }));
    }
}
```

---

## 11. Deployment

### 11.1 Single Binary (giống snake-rs)

```rust
// Cargo.toml
[dependencies]
rust-embed = "8"

// main.rs
#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;
```

### 11.2 Docker

```dockerfile
FROM rust:1.75-alpine AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM alpine:latest
COPY --from=builder /app/target/release/thirteen-rs /usr/local/bin/
EXPOSE 3000
CMD ["thirteen-rs"]
```

---

## 12. Scaling Considerations

### 12.1 In-Memory (MVP)
- Single server
- DashMap cho rooms
- Broadcast channel per room

### 12.2 Redis (Production)
```rust
pub struct RedisBroadcaster {
    client: redis::Client,
}

impl Broadcaster for RedisBroadcaster {
    // Pub/Sub per room channel
    // "room:{code}:events"
}
```

### 12.3 Room Distribution
- Consistent hashing cho room → server mapping
- Sticky sessions via room code

---

## 13. Implementation Roadmap

### Phase 1: Core (Week 1)
- [ ] Card, Deck, Hand structs
- [ ] Combo validation
- [ ] Room creation/join

### Phase 2: Gameplay (Week 2)
- [ ] Turn management
- [ ] Play validation
- [ ] Win condition

### Phase 3: WebSocket (Week 3)
- [ ] Protocol messages
- [ ] Real-time updates
- [ ] Room broadcasts

### Phase 4: Frontend (Week 4)
- [ ] Card rendering
- [ ] Selection UI
- [ ] Game flow

### Phase 5: Polish (Week 5)
- [ ] Anti-cheat integration
- [ ] Event logging
- [ ] Error handling
- [ ] Testing

---

## 14. Kết Luận

Dự án **Thirteen Card Game** có thể tái sử dụng nhiều pattern từ `snake-rs`:

| Component | Reuse Level | Notes |
|-----------|-------------|-------|
| WebSocket handler | 80% | Thêm room-scoped logic |
| Broadcaster | 60% | Cần room-level channels |
| Rate Limiter | 100% | Giữ nguyên |
| Event Logger | 70% | Thêm card game events |
| State structure | 50% | Rooms thay vì flat snakes |
| Protocol | 20% | Messages hoàn toàn khác |
| Game logic | 0% | Viết mới hoàn toàn |

Ước tính effort: **3-5 tuần** cho MVP với 1 developer.
