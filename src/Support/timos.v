//===============================================================================================================================
//
//    Copyright © 2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : NEWPORT
//
//    Description          : timos in and out from All Tiles to TileZero
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//    Notes:
//
//
//===============================================================================================================================

module  timos #(
   parameter             BASE     = 13'h00000,
   parameter             NUM_REGS = 1
   )(
   // A2S I/O interface
   output wire [ 32:0]  imiso,         // Slave data out {               IOk, IOo[31:0]}
   input  wire [ 47:0]  imosi,         // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
   // timos I/O
   input  wire [255:0]  timo_in, //JLPL - 11_11_25
   // Global
   output wire          intpt,
   input  wire          reset,
   input  wire          clock
   );

localparam  timo_IN = 0, timo_OUT = 1;

// Interface decoding ===========================================================================================================

wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
wire            IOk;
// Decollate imosi
assign   {cIO, iIO, aIO} = imosi;

wire         in_range =  aIO[12:0] == BASE[12:0];
wire  [1:0]    decode =   in_range; //JLPL - 11_11_25 - missing semi-colon
wire  [1:0]    rdNclr =  {NUM_REGS{(cIO==3'b100)}} & decode;
wire  [1:0]      read =  {NUM_REGS{(cIO==3'b101)}} & decode;

// Locals =======================================================================================================================

reg   [ 31:0]  R_timo_OUT;

wire  [255:0]  timo_dec;
wire  [255:0]  sync_timo; //JLPL - 11_11_25
//wire  [256:1]  next_timo;  // Extra bit is wraparound //JLPL - 11_5_25
wire  [  8:0]  next_timo;    // Extra bit is wraparound //JLPL - 11_5_25
wire  [ 31:0]  timo_data;
wire  [  7:0]  tnum_do, tnum_di;
wire           any_timo, sync_any_timo, dlyd_any_timo;
wire           tnum_wr, tnum_rd;
wire           tnum_or, tnum_ir;
wire           sync_clock;

genvar   i;

// Registers ====================================================================================================================
//
// Each tile outputs a timo which is used to create an interrupt into Tile Zero.  As each Tile One is in its own clock domain
// each timo must be individually sychronized to the Tile Zero clock.  So we first OR all the timos and feed the result into a
// 3 stage synchonizer.  We then use that to enable 256 synchronizers to capture
// timo inputs from each Tile1.
//
// Sychronize to local clock through 3 flops.  First for the OR of all timos
assign
   any_timo = |timo_in[255:0]; //JLPL - 11_11_25

A2_sync3 i_S31 (.q(sync_any_timo), .d(     any_timo), .reset(reset), .clock(clock));
A2_sync3 i_S32 (.q(dlyd_any_timo), .d(sync_any_timo), .reset(reset), .clock(clock));

//  ...and then use the OR as a clock enable to synchronize them individually

`ifdef synthesis
       PREICG_X2N_A7P5PP84TR_C14 iCGxx ( .CK(clock), .E(sync_any_timo), .SE(1'b0), .ECK(sync_clock) );
`else
preicg i_CG (.ECK(sync_clock), .E(sync_any_timo), .SE(1'b1), .CK(clock));
`endif

for (i=0; i<256; i=i+1) begin : SF //JLPL - 11_11_25
   A2_sync3 i_SF (.q(sync_timo[i]), .d(timo_in[i]),  .reset(reset), .clock(sync_clock));
   end

A2_arbiter_enc #(256,8) iARB (
   .win  ( next_timo[  8:0]   ),       // Encoded winner of arbitration : msb is 'found' //JLPL - 11_5_25 - Fix missing comma
   .req  ( sync_timo[255:0]   ),       // Incoming Requests                              //JLPL - 11_5_25 - Fix missing comma and add concatenation
   .adv  ( tnum_or & tnum_rd  ),                         // Request taken!  Reload Last Register. Tie to '1' if always taken.
   .en   ( tnum_ir & any_timo ),       // Global Enable:  if false no win!!
   .clock(clock               ),
   .reset(reset               )
   );

assign   tnum_wr = next_timo[8];
//    1. On interrupt read pipe output but do not pop. (read only)
//    2. Service the interrupt
//    3. Clear the timo at the source
//    4. Pop the output (read and clear)

assign tnum_di = next_timo[7:0]; //JLPL - 11_5_25

A2_pipeONE p1 (
   .pdi      (tnum_di  ), //JLPL - 11_5_25
   .pwr      (tnum_wr  ), //JLPL - 11_5_25
   .pir      (tnum_ir  ), //JLPL - 11_5_25

   .pdo      (tnum_do  ), //JLPL - 11_5_25
   .por      (tnum_or  ), //JLPL - 11_5_25
   .prd      (tnum_rd  ), //JLPL - 11_5_25

   .clock    (clock),
   .reset    (reset)
   );

assign
   timo_data   = !tnum_or ? 32'h0 : {1'b1,23'h0,tnum_do[7:0]},
   tnum_rd     =  rdNclr[timo_IN];                             // Read-and-clear to pop the FIFO

// Processor can read the FIFO output without popping the value or it may use a Read-&-clear to pop
// the value from the FIFO.   MSB is deasserted if there is a valid entry and asserted (negative) if empty

// Output interface =============================================================================================================

assign
   IOo   = {24'h0,read ? tnum_do[7:0] : 8'h0},
   IOk   =  decode & cIO[2],
   imiso = {IOk, IOo},
   intpt =  tnum_or; //JLPL - 11_11_25

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
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
