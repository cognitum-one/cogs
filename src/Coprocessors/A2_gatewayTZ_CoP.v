/* ******************************************************************************************************************************

   Copyright © 2025 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : COGNITUM

   Description          : Tile Output -- A2S creates access to the rest of the chip -- Includes PUF Oscillator access for RoT

*********************************************************************************************************************************
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*********************************************************************************************************************************

PROTOTYPE:

A2_gatewayTZ_CoP #(
   .BASE    (13'h00000),      // Base Address of Co-processor
   .NUM_REGS(12)
   ) **instance name** (
   .miso       (miso),        // Tile Outgoing to CDCo
   .ordy       (ordy),        // CoP is ready to send
   .pop        (pop),         // CDC takes bmiso
   .mosi       (mosi),        // Tile Response Incoming from iCDC
   .push       (push),        // CDC returns Response
   .irdy       (irdy),        // CoP is ready to receive
   .imiso      (imiso),       // Slave data out {                IOk, IOo[31:0]}
   .imosi      (imosi),       // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
   .intpt_fini (intpt_fini),  // Access to Raceway Finished -- Response received
   .intpt_timo (intpt_timo),  // Timed-out
   .myID       (myID),        // Tile ID
   .pufvector  (pufvector),   // PUF oscillator select vectors
   .puf_enable (puf_enable),  // PUF enable
   .flag       (flag),        // flag A2S
   .reset      (reset),
   .clock      (clock)
   );

********************************************************************************************************************************/

`ifndef tCQ
`define tCQ #1
`endif

module   A2_gatewayTZ_CoP #(
   parameter   [12:0]   BASE     =  13'h00000,  // Base Address of Co-processor
   parameter            NUM_REGS =  12
   )(
   // Request out to Raceway
   output wire [95:0]   miso,       // Tile Outgoing to CDCo
   output wire          ordy,       // CoP is ready to send
   input  wire          pop,        // CDC takes bmiso
   // Response in from Raceway
   input  wire [95:0]   mosi,       // Tile Response Incoming from iCDC
   input  wire          push,       // CDC returns Response
   output wire          irdy,       // CoP is ready to receive
   // A2S IO Interface
   output wire [32:0]   imiso,      // Slave data out {                IOk, IOo[31:0]}
   input  wire [47:0]   imosi,      // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
   output wire          intpt_fini, // Access to Raceway Finished -- Response received
   output wire          intpt_timo, // Timed-out
   input  wire [ 7:0]   myID,
   // PUF control                   // 333333222_222222211_111111110_000000000
   input  wire [35:0]   pufvector,  // 543210987_654321098_765432109_876543210
   input  wire          puf_enable,
   // Global
   output wire          flag,       // flag A2S
   input  wire          reset,
   input  wire          clock
   );

localparam  LNRG  =  4; // `LG2(NUM_REGS);

`ifdef   RELATIVE_FILENAMES
   `include "COGNITUM_IO_addresses.vh"
`else
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_IO_addresses.vh"
`endif

// Physical Registers ===========================================================================================================

reg   [        31:0] CMND, ADDR, DATA, RDA1, RDA0, RESP, TIMB, TIMR, FLAG;
reg   [         7:0] RXDONE;
reg   [         4:0] EFSM;
reg   [         3:0] TFSM;
reg   [         2:0] RFSM;
reg                  BUSY, TIMO, LAST;

// Nets
wire  [NUM_REGS-1:0] decode, read, write;
wire  [        95:0] PUFaccess,GWYaccess; //JLPL - 9_28_25
wire  [        31:0] iIO, IOo, PUFaddr;
wire  [        12:0] aIO;
wire  [        19:0] ena01, ena02, ena03, ena12, ena13, ena23, ena0, ena1, ena2, ena3;
wire  [        19:0] run01, run02, run03, run12, run13, run23, run0, run1, run2, run3;
wire  [        19:0] acc0, acc1, acc2, acc3;
wire  [         7:0] tagin;
wire  [         2:0] cIO;
wire                 fred;
wire                 allowed;
wire                 complete;
wire                 win, step0;
wire                 GWYgiven, PUFgiven, PUFsend;
wire                 GWYidle,  GWYsend, GWYrspn, GWYfini;
wire                 hit_01, hit_02, hit_03, hit_12, hit_13, hit_23;
wire                 hit_0,  hit_1,  hit_2,  hit_3;
wire                 done1,  done2;
wire                 IOk;
wire                 ld_ADDR, ld_DATA, ld_CMND, ld_RDA1, ld_RDA0, ld_RESP, ld_TIMO, ld_TIMB, ld_TIMR, ld_FLAG, ld_BUSY;
wire                 merged, PUFrspn;
wire                 time0, takePUF, takeGWY;
wire                 wr, rd, sw, rs, ws, in_range;

// Interface decoding -----------------------------------------------------------------------------------------------------------

assign
   // Decollate imosi
   {cIO[2:0], iIO[31:0], aIO[12:0]} = imosi,
   // decodes
   wr       = cIO==3'b110,
   rd       = cIO==3'b101,
   sw       = cIO==3'b111,       // swap
   rs       = rd | sw,
   ws       = wr | sw,
   in_range = aIO[12:LNRG] == BASE[12:LNRG],
   decode   = in_range << aIO[LNRG-1:0],
   write    = {NUM_REGS{wr}} & decode,
   read     = {NUM_REGS{rd}} & decode,
   // Security check
   PUFaddr  =  Regs + ((TILE_base + TILE_PUF_OSCS) << 2),
   allowed  =  ADDR != PUFaddr,
   // Writes
   ld_ADDR  = write[GATEWAY_ADDR],              // Push Address
   ld_DATA  = write[GATEWAY_DATA],              // Push Data
   ld_CMND  = write[GATEWAY_CMND] & allowed,    // Push Command -- kicks off external access
   ld_RDA1  = write[GATEWAY_RDA1],              // Read Data 1
   ld_RDA0  = write[GATEWAY_RDA0],              // Read Data 0
   ld_RESP  = write[GATEWAY_RESP],              // Response
   ld_TIMO  = write[GATEWAY_TIMO],              // TimeOut
   ld_TIMB  = write[GATEWAY_TIMB],              // TimeBase
   ld_TIMR  = write[GATEWAY_TIMR],              // Timer
   ld_FLAG  = write[GATEWAY_FLAG],              // Flag
   ld_BUSY  = ws & decode[GATEWAY_BUSY];        // Write or Swap BUSY  0: Free -1: BUSY

// Registers -------------------------------------------------------------------------------------------------------------------

assign
   time0    =  ~|TIMR[31:0];

always @(posedge clock)    if ( reset              )  TIMB  <= `tCQ {32{1'b1}};
                      else if (            ld_TIMB )  TIMB  <= `tCQ  iIO[31:0];

always @(posedge clock)    if ( reset              )  TIMR  <= `tCQ {32{1'b1}};
                      else if ( GWYidle && ld_TIMR )  TIMR  <= `tCQ  iIO;
                      else if ( GWYidle && ld_CMND )  TIMR  <= `tCQ  TIMB;
                      else if ( GWYrspn && !time0  )  TIMR  <= `tCQ  TIMR -1;

always @(posedge clock)    if ( GWYidle && ld_ADDR )  ADDR  <= `tCQ  iIO[31:0];
always @(posedge clock)    if ( GWYidle && ld_DATA )  DATA  <= `tCQ  iIO[31:0];
always @(posedge clock)    if ( GWYidle && ld_CMND )  CMND  <= `tCQ {iIO[31:24],1'b0,iIO[22:8],myID[7:0]};

always @(posedge clock)    if ( GWYfini || reset   )  BUSY  <= `tCQ  1'b0;
                      else if ( GWYidle && ld_BUSY )  BUSY  <= `tCQ  iIO[0];
                      else if ( GWYidle && ld_CMND )  BUSY  <= `tCQ  1'b1;

always @(posedge clock)    if ( GWYidle && ld_RDA1 )  RDA1  <= `tCQ   iIO[31:0];
                      else if ( GWYfini            )  RDA1  <= `tCQ  mosi[31:0];

always @(posedge clock)    if ( GWYidle && ld_RDA0 )  RDA0  <= `tCQ   iIO[31:0];
                      else if ( GWYfini            )  RDA0  <= `tCQ  mosi[63:32];

always @(posedge clock)    if ( GWYidle && ld_RESP )  RESP  <= `tCQ   iIO[31:0];
                      else if ( GWYfini            )  RESP  <= `tCQ {mosi[95:88],1'b0,mosi[86:64]};

always @(posedge clock)    if ( reset              )  TIMO  <= `tCQ 1'b0;
                      else if ( GWYrspn &&   ~TIMO )  TIMO  <= `tCQ time0;       // Sets once and holds!
                      else if (            ld_TIMO )  TIMO  <= `tCQ iIO[0];      // until cleared here

always @(posedge clock)    if ( reset              )  FLAG  <= `tCQ 32'h0;
                      else if (            ld_FLAG )  FLAG  <= `tCQ iIO[0];

// imiso outputs-----------------------------------------------------------------------------------------------------------------

A2_mux #(12,32) iIOO (.z(IOo[31:0]), .s(read),
                      .d({ FLAG,
                           TIMR,
                           TIMB,
                           {32{TIMO}},
                           {RESP & 32'hff7fffff},
                           RDA0,
                           RDA1,
                           {32{BUSY}},
                           {CMND & 32'hff7fffff},
                           DATA,
                           ADDR,
                           32'h0
                           }));

assign
   IOk      =  (rs | ws) & in_range,
   imiso    =  {IOk, IOo};

// Gateway access to Raceway FSM ------------------------------------------------------------------------------------------------

localparam  [2:0]  IDLE = 0,         WAIT = 1,         SEND = 2,         RSPN = 3,         FINI = 4;
localparam  [4:0] sIDLE = 5'b00001, sWAIT = 5'b00010, sSEND = 5'b00100, sRSPN = 5'b01000, sFINI = 5'b10000;

always @(posedge clock or posedge reset)
   if (reset)                      EFSM  <= `tCQ sIDLE;
   else case (1'b1)
      EFSM[IDLE] : if ( ld_CMND  ) EFSM  <= `tCQ sWAIT;    // kicks off the FSM for a PUF access
      EFSM[WAIT] : if (!takeGWY  ) EFSM  <= `tCQ sSEND;    // if the coast is clear send a request
      EFSM[SEND] : if ( takeGWY  ) EFSM  <= `tCQ sRSPN;    // Wait for the request to be taken
      EFSM[RSPN] : if ( time0    ) EFSM  <= `tCQ sIDLE;    // Wait for a response.   Timeout: give up!!
              else if ( GWYgiven ) EFSM  <= `tCQ sFINI;    // Wait for a response
      EFSM[FINI] :                 EFSM  <= `tCQ sIDLE;    // Send an interrupt to the processor and goto idle
      default                      EFSM  <= `tCQ sIDLE;
      endcase

assign
   GWYidle     =  EFSM[IDLE],
   GWYfini     =  EFSM[FINI],
   GWYsend     =  EFSM[SEND],
   GWYrspn     =  EFSM[RSPN],
   GWYgiven    =  push     & ~mosi[95] & ~mosi[87],
   PUFgiven    =  push     & ~mosi[95] &  mosi[87],
   irdy        =  GWYrspn  & ~mosi[95] & ~mosi[87]             // Gateway
               |  PUFrspn  & ~mosi[95] &  mosi[87],            // PUF
   GWYaccess   = {CMND,DATA,ADDR};

// PUF State Machine ------------------------------------------------------------------------------------------------------------

// PUF oscillator control FSM
// PUF Security:  To ensure that PUF accesses cannot be traced we use the tags field.   The m.s. tag bit cannot be set under
// Gateway accesses (always zero)  so the CMND and RESP registers always show bit-23 as zero.  The PUF accesses set the tag bit
// to a one on the raceway so they can 'find their way home'.  The lower tags bits are also used to track the PUF accesses.
//
// As there are 2 oscillators per Tile, detect if any Tile is selected twice, i.e. for both its oscillators.
// Combine any double hits into a single accesses and keep track of the channels loaded ('DONE').  This removes the
// need for read-modify-writes which would take much longer.
//                       7 6 5 4 3 2 1 0
//                      +-+-+---+-+-+---+    R: Run the oscillator
//                      |E|R| A |E|R| A |    E: Enable power to the oscillator
//                      +-+-+---+-+-+---+    A: Address of lane to put output onto

assign
   hit_01   =  pufvector[ 8:1 ] == pufvector[17:10],     // Detect any double hits for this tile
   hit_02   =  pufvector[ 8:1 ] == pufvector[26:19],
   hit_03   =  pufvector[ 8:1 ] == pufvector[35:28],
   hit_12   =  pufvector[17:10] == pufvector[26:19],
   hit_13   =  pufvector[17:10] == pufvector[35:28],
   hit_23   =  pufvector[26:19] == pufvector[35:28],
   hit_0    = ~hit_01 & ~hit_02 & ~hit_03,               // Singles
   hit_1    = ~hit_01 & ~hit_12 & ~hit_13,
   hit_2    = ~hit_02 & ~hit_12 & ~hit_23,
   hit_3    = ~hit_03 & ~hit_13 & ~hit_23,
   // Enable:  Power-on Tile Oscillators
   ena01    = {pufvector[ 8:1 ], pufvector[00] ? 12'b0011_1000_1001 : 12'b0011_1001_1000},
   ena02    = {pufvector[ 8:1 ], pufvector[00] ? 12'b0101_1000_1010 : 12'b0101_1010_1000},
   ena03    = {pufvector[ 8:1 ], pufvector[00] ? 12'b1001_1000_1011 : 12'b1001_1011_1000},
   ena12    = {pufvector[17:10], pufvector[09] ? 12'b0110_1001_1010 : 12'b0110_1010_1001},
   ena13    = {pufvector[17:10], pufvector[09] ? 12'b1010_1001_1011 : 12'b1010_1011_1001},
   ena23    = {pufvector[26:19], pufvector[18] ? 12'b1100_1010_1011 : 12'b1100_1011_1010},
   ena0     = {pufvector[ 8:1 ], pufvector[00] ? 12'b0001_1000_0000 : 12'b0001_0000_1000},
   ena1     = {pufvector[17:10], pufvector[09] ? 12'b0010_1001_0000 : 12'b0010_0000_1001},
   ena2     = {pufvector[26:19], pufvector[18] ? 12'b0100_1010_0000 : 12'b0100_0000_1010},
   ena3     = {pufvector[35:28], pufvector[27] ? 12'b1000_1011_0000 : 12'b1000_0000_1011},
   // Run Tile Oscillators
   run01    = {pufvector[ 8:1 ], pufvector[00] ? 12'b0011_1100_1101 : 12'b0011_1101_1100},
   run02    = {pufvector[ 8:1 ], pufvector[00] ? 12'b0101_1100_1110 : 12'b0101_1110_1100},
   run03    = {pufvector[ 8:1 ], pufvector[00] ? 12'b1001_1100_1111 : 12'b1001_1111_1100},
   run12    = {pufvector[17:10], pufvector[09] ? 12'b0110_1101_1110 : 12'b0110_1110_1101},
   run13    = {pufvector[17:10], pufvector[09] ? 12'b1010_1101_1111 : 12'b1010_1111_1101},
   run23    = {pufvector[26:19], pufvector[18] ? 12'b1100_1110_1111 : 12'b1100_1111_1110},
   run0     = {pufvector[ 8:1 ], pufvector[00] ? 12'b0001_1100_0000 : 12'b0001_0000_1100},
   run1     = {pufvector[17:10], pufvector[09] ? 12'b0010_1101_0000 : 12'b0010_0000_1101},
   run2     = {pufvector[26:19], pufvector[18] ? 12'b0100_1110_0000 : 12'b0100_0000_1110},
   run3     = {pufvector[35:28], pufvector[27] ? 12'b1000_1111_0000 : 12'b1000_0000_1111},
   // Accesses types
   acc0     =  hit_01                             ?  ( step0 ? ena01 : run01)
            :  hit_02                             ?  ( step0 ? ena02 : run02)
            :  hit_03                             ?  ( step0 ? ena03 : run03)
            :  hit_12                             ?  ( step0 ? ena12 : run12)
            :  hit_13                             ?  ( step0 ? ena13 : run13)
            :  hit_23                             ?  ( step0 ? ena23 : run23)
            :  hit_0                              ?  ( step0 ? ena0  : run0 ) : 12'h0,
   acc1     =  hit_01 && hit_23                   ?  ( step0 ? ena23 : run23)             // done1
            :  hit_02 && hit_13                   ?  ( step0 ? ena13 : run13)             // done1
            :  hit_03 && hit_12                   ?  ( step0 ? ena12 : run12)             // done1
            :  hit_01 && hit_2                    ?  ( step0 ? ena2  : run2 )
            :  hit_02 && hit_1                    ?  ( step0 ? ena1  : run1 )
            :  hit_03 && hit_1                    ?  ( step0 ? ena1  : run1 )
            :  hit_12 && hit_0                    ?  ( step0 ? ena0  : run0 )
            :  hit_13 && hit_0                    ?  ( step0 ? ena0  : run0 )
            :  hit_23 && hit_0                    ?  ( step0 ? ena0  : run0 )
            :  hit_0  && hit_1                    ?  ( step0 ? ena1  : run1 ) : 12'h0,
   acc2     =  hit_01 && hit_2 && hit_3           ?  ( step0 ? ena3  : run3 )             // done2
            :  hit_02 && hit_1 && hit_3           ?  ( step0 ? ena3  : run3 )             // done2
            :  hit_12 && hit_0 && hit_3           ?  ( step0 ? ena3  : run3 )             // done2
            :  hit_13 && hit_0 && hit_2           ?  ( step0 ? ena2  : run2 )             // done2
            :  hit_23 && hit_0 && hit_1           ?  ( step0 ? ena1  : run1 )             // done2
            :  hit_03 && hit_1 && hit_2           ?  ( step0 ? ena2  : run2 )             // done2
            :  hit_0  && hit_1 && hit_2           ?  ( step0 ? ena2  : run2 ) : 12'h0,
   acc3     =  hit_0  && hit_1 && hit_2 && hit_3  ?  ( step0 ? ena3  : run3 ) : 12'h0,
   // Progress signals
   done1    =  hit_01 & hit_23 | hit_02 &  hit_13 | hit_03 &  hit_12,
   done2    =  hit_01 & hit_2  & hit_3  |  hit_02 & hit_1  & hit_3  | hit_12 & hit_0  & hit_3
            |  hit_13 & hit_0  & hit_2  |  hit_23 & hit_0  & hit_1  | hit_03 & hit_1  & hit_2;

// This state machine initates PUF control accesses
localparam  [3:0] EPUF0 = 0, EPUF1 = 1, EPUF2 = 2,  EPUF3 = 3,  RPUF0 = 4,  RPUF1 = 5, RPUF2 = 6, RPUF3 = 7,
                  ENPUF = 8, RUNPF = 9, SENT  = 10, TOFF  = 11, MERGE = 12,                       NULL  = 15;

always @(posedge clock)
   if (reset)                   TFSM   <= `tCQ  NULL;
   else case (TFSM)
      NULL  :  if ( puf_enable) TFSM   <= `tCQ  ENPUF;
      ENPUF :                   TFSM   <= `tCQ  EPUF0;
      EPUF0 :  if ( takePUF  )  TFSM   <= `tCQ  EPUF1;
      EPUF1 :  if ( takePUF  )  TFSM   <= `tCQ  EPUF2;
      EPUF2 :  if ( done1    )  TFSM   <= `tCQ  RUNPF;
          else if ( takePUF  )  TFSM   <= `tCQ  EPUF3;
      EPUF3 :  if ( done2    )  TFSM   <= `tCQ  RUNPF;
          else if ( takePUF  )  TFSM   <= `tCQ  RUNPF;
      RUNPF :                   TFSM   <= `tCQ  RPUF0;
      RPUF0 :  if ( takePUF  )  TFSM   <= `tCQ  RPUF1;
      RPUF1 :  if ( takePUF  )  TFSM   <= `tCQ  RPUF2;
      RPUF2 :  if ( done1    )  TFSM   <= `tCQ  SENT;
          else if ( takePUF  )  TFSM   <= `tCQ  RPUF3;
      RPUF3 :  if ( done2    )  TFSM   <= `tCQ  SENT;
          else if ( takePUF  )  TFSM   <= `tCQ  SENT;
      SENT  :  if (!puf_enable) TFSM   <= `tCQ  TOFF;
      TOFF  :  if ( takePUF  )  TFSM   <= `tCQ  MERGE;
      MERGE :  if ( merged   )  TFSM   <= `tCQ  NULL;
      default                   TFSM   <= `tCQ  NULL;
      endcase

assign                                        //   cmd> <---tag---->  <--dest--->  <src>  <-----data----->  <----addr--->
   PUFaccess   =  TFSM == EPUF0              ?  {12'h91_8,acc0[11:8], acc0[19:12], 8'h00, 24'h0, acc0[7:0], PUFaddr[31:0]}
               :  TFSM == EPUF1              ?  {12'h91_8,acc1[11:8], acc1[19:12], 8'h00, 24'h0, acc1[7:0], PUFaddr[31:0]}
               : (TFSM == EPUF2) && !done1   ?  {12'h91_8,acc2[11:8], acc2[19:12], 8'h00, 24'h0, acc2[7:0], PUFaddr[31:0]}
               :  TFSM == EPUF3              ?  {12'h91_8,acc3[11:8], acc3[19:12], 8'h00, 24'h0, acc3[7:0], PUFaddr[31:0]}
               :  TFSM == RPUF0              ?  {12'h91_9,acc0[11:8], acc0[19:12], 8'h00, 24'h0, acc0[7:0], PUFaddr[31:0]}
               :  TFSM == RPUF1              ?  {12'h91_9,acc1[11:8], acc1[19:12], 8'h00, 24'h0, acc1[7:0], PUFaddr[31:0]}
               : (TFSM == RPUF2) && !done1   ?  {12'h91_9,acc2[11:8], acc2[19:12], 8'h00, 24'h0, acc2[7:0], PUFaddr[31:0]}
               : (TFSM == RPUF3) && !done2   ?  {12'h91_9,acc3[11:8], acc3[19:12], 8'h00, 24'h0, acc3[7:0], PUFaddr[31:0]}
               :  TFSM == TOFF               ?  {12'hb1_a,    4'h00 ,      8'h00 , 8'h00, 32'h0,            PUFaddr[31:0]}
               :                                 96'h00_00_00_00_00000000_00000000,
   step0       =  TFSM <  RPUF0,
   PUFsend     =  PUFaccess[95],
   tagin       =  mosi[83:80] << (mosi[84] ? 4 : 0),
   complete    = &RXDONE;

// This state machine collects returning responses from the above requests
localparam  [2:0] LAZY = 0, ACKN = 1, HOLD = 2,  DONE = 3,  UNITE = 4;

always @(posedge clock)
   if (reset)                    {RFSM,RXDONE}  <= `tCQ  {NULL, 8'h00};
   else case (RFSM)
      LAZY  :  if ( puf_enable )  RFSM          <= `tCQ   ACKN;
      ACKN  :  if ( complete   )  RFSM          <= `tCQ   HOLD;
          else if ( PUFgiven   )       RXDONE   <= `tCQ   RXDONE | tagin[7:0];
      HOLD  :  if (!puf_enable )  RFSM          <= `tCQ   DONE;
      DONE  :  if ( PUFgiven   ) {RFSM,RXDONE}  <= `tCQ  {UNITE,8'h00};
      UNITE :  if ( merged     )  RFSM          <= `tCQ   LAZY;
      default                     RFSM          <= `tCQ   LAZY;
      endcase

assign
   PUFrspn  = (RFSM == ACKN ) | (RFSM == DONE ),
   merged   = (RFSM == UNITE) & (TFSM == MERGE);

// Gateway Outputs to Raceway ---------------------------------------------------------------------------------------------------

assign
   flag        =  FLAG[0],
   intpt_fini  =  GWYfini,
   intpt_timo  =  TIMO,
   win         =  LAST     ? ~GWYsend         : PUFsend,    // 0: GWYsend 1: PUFsend
   takeGWY     = ~win      &  pop,
   takePUF     =  win      &  pop,
   ordy        =  GWYsend  |  PUFsend,
   miso[95:0]  =  win      ?  PUFaccess[95:0] : GWYaccess[95:0];

always @(posedge clock)
   if       (reset)        LAST  <= `tCQ  1'b0;
   else if  (pop && ordy)  LAST  <= `tCQ  win;


endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_gateway_TZ
module t;

parameter   [12:0]         BASE = 13'h00000;  // Base Address of Co-processor
parameter              NUM_REGS = 12;

`include "COGNITUM_IO_addresses.vh"

// INPUTS TO MUT
// Request out to Raceway
reg            pop;        // CDC takes bmiso
// Response in from Raceway
reg   [95:0]   mosi;       // Tile Response Incoming from iCDC
reg            push;       // CDC returns Response
// A2S IO Interface
reg   [47:0]   imosi;      // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
reg   [ 7:0]   myID;
// PUF Oscillator Interface
reg   [35:0]   pufvector;
reg            puf_enable;
// Global
reg            reset;
reg            clock;

// OUTPUTS FROM MUT
// Request out to Raceway
wire  [95:0]   miso;       // Tile Outgoing to CDCo
wire           ordy;       // CoP is ready to send
// Response in from Raceway
wire           irdy;       // CoP is ready to receive
// A2S IO Interface
wire  [32:0]   imiso;      // Slave data out {                IOk, IOo[31:0]}
wire           intpt_fini; // Access to Raceway Finished -- Response received
wire           intpt_timo; // Timed-out
// Global
wire           flag;       // flag A2S

A2_gatewayTZ_CoP #(
   .BASE    (GATEWAY_base),      // Base Address of Co-processor
   .NUM_REGS(12)
   ) mut (
   .miso       (miso),        // Tile Outgoing to CDCo
   .ordy       (ordy),        // CoP is ready to send
   .pop        (pop),         // CDC takes bmiso
   .mosi       (mosi),        // Tile Response Incoming from iCDC
   .push       (push),        // CDC returns Response
   .irdy       (irdy),        // CoP is ready to receive
   .imiso      (imiso),       // Slave data out {                IOk, IOo[31:0]}
   .imosi      (imosi),       // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
   .intpt_fini (intpt_fini),  // Access to Raceway Finished -- Response received
   .intpt_timo (intpt_timo),  // Timed-out
   .myID       (myID),
   .pufvector  (pufvector),   // PUF oscillator select vectors
   .puf_enable (puf_enable),  // PUF enable
   .flag       (flag),        // flag A2S
   .reset      (reset),
   .clock      (clock)
   );

initial begin
   clock = 1'b0;
   forever
      clock = #50 ~clock;
   end

initial begin
   pop         =  1'b0;
   mosi        =  1'b0;
   push        =  1'b0;
   imosi       = 48'h0;
   myID        =  8'h0;
   pufvector   = 36'h0;
   puf_enable  =  1'b0;
   reset       =  1'b0;
   repeat (5)  @(posedge clock);
   $display("  Resetting Gateway");
   reset       =  1'b1;

   fork : Split
      begin : reSet //-----------------------------------------------------------------------
      repeat (5) @(posedge clock);
      reset = 1'b0;
      end

      begin : MISOoutput //------------------------------------------------------------------
      integer a;
      a = 0;
      while (reset) @(posedge clock);
      repeat (35)   @(posedge clock);
      $display("  Trying PUF state machine");

      pop         =  1'b0;
      pufvector   =  36'h000001208; //{8'h01,1'b0,  8'h02,1'b0,  8'h03,1'b0,  8'h04,1'b0}; //
      puf_enable  =  1'b1;
      pop         =  1'b1;

      while (!mut.complete) @(posedge clock);
      repeat (300)      @(posedge clock);
      puf_enable  =  1'b0;
      end

      begin : Gateway_side
      integer i;
      reg [12:0] addr;
      reg [31:0] data;
      repeat (20) @(posedge clock);
      for (i=0; i<64; i=i+4) begin
         while (mut.BUSY) @(posedge clock)
         repeat (1) @(posedge clock);
         addr  =  GATEWAY_base + GATEWAY_ADDR;
         data  =  32'h40000004 + i;
         repeat (1)  @(posedge clock);
         imosi = { 3'b110, data, addr};      // ADDR
         repeat (1)  @(posedge clock);
         imosi =  48'h0;
         addr  =  GATEWAY_base + GATEWAY_DATA;
         imosi = { 3'b110, 32'h55555555, addr };         // DATA
         repeat (1)  @(posedge clock);
         imosi =  48'h0;
         addr  =  GATEWAY_base + GATEWAY_CMND;
         imosi = { 3'b110, 32'h91000100, addr };         // CMND
         repeat (1)  @(posedge clock);
         imosi =  48'h0;
         repeat (100)   @(posedge clock);
         $display($time,"Gateway Access #%2d",i);
         end
      end

      begin : RAM_responses
      integer j;
      reg  active;
      while (reset) begin
         j      = 0;
         active = 1'b0;
         @(posedge clock);
         end
      while (j < 16) begin
         if (miso[95] && ordy) begin
            active= 1'b1;
            pop   = 1'b1;
            repeat (1)  @(posedge clock);
            pop   = 1'b0;
            repeat (2)  @(posedge clock);
            mosi  = miso & 96'h7fff00ff_ffffffff_ffffffff;
            while(!irdy) @(posedge clock)
            push  = 1'b1;
            repeat (1)  @(posedge clock);
            push  = 1'b0;
            mosi  = 96'h0;
            end
         else begin
            while (!(miso[96] && ordy))  @(posedge clock);
            active= 1'b0;
            end
         @(posedge clock);
         j = j + 1;
         end
      end


   join

   repeat (500)   @(posedge clock);
   $display("  Finishing");
   puf_enable  =  1'b0;
   repeat (50) @(posedge clock);
   $stop;
   end

endmodule
`endif
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

