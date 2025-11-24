//==============================================================================================================================
//
//    Copyright (c) 2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : COGNITUM
//
//    Description          : TileZero Power-On Reset and start-up mechanism
//
//==============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//==============================================================================================================================
//    Notes for Use and Synthesis:
//
//     31                                              7 6 5 4 3 2 1 0
//    +-----------------------------------------------+-+-------+-----+
//    |                 read as zeros                 |E|   D   |  F  |    TCC
//    +-----------------------------------------------+/+---/---+--/--+
//             E:  Enable     Turn Oscillator on/off _/    /      /
//             D:  Divisor     1..16 integer divide ______/      /
//             F:  Frequency  16..23 GHz  ______________________/
//
//=============================================================================================================================*/

module TileZero_boot (
   output wire       ref_reset,
   output wire       ref_clock,
   output wire       div_clock,
   output wire       boot_reset,
   output wire       boot,
   output wire       ring_oscillator_reset,     // JLPL - 11_7_25
   input  wire [10:0] TCC,
   input  wire       PoR_bypass,                // Power-On Reset bypass
   input  wire       PoR_extern,                // Power-On Reset external when bypassed
   input  wire       PoR,                       // Power-On Reset
   input  wire       done                       // Boot load of Coder RAM complete
   );

localparam [255:0]  K1  = {
   //   Clock-En Feedback                     mark:space
   16'b1111111101111111,    // divide by 16    8:8     50.00% //JL
   16'b1111111100111111,    // divide by 15    8:7     53.33% //JL
   16'b0111111110111111,    // divide by 14    7:7     50.00% //JL
   16'b0111111110011111,    // divide by 13    7:6     53.84% //JL
   16'b0011111111011111,    // divide by 12    6:6     50.00% //JL
   16'b0011111111001111,    // divide by 11    6:5     54.54% //JL
   16'b0001111111101111,    // divide by 10    5:5     50.00% //JL
   16'b0001111111100111,    // divide by 9     5:4     55.55% //JL
   16'b0000111111110111,    // divide by 8     4:4     50.00% //JL
   16'b0000111111110011,    // divide by 7     4:3     57.14% //JL
   16'b0000011111111011,    // divide by 6     3:3     50.00% //JL
   16'b0000011111111001,    // divide by 5     3:2     60.00% //JL
   16'b0000001111111101,    // divide by 4     2:2     50.00% //JL
   16'b0000001111111100,    // divide by 3     2:1     66.66% //JL
   16'b0000000111111110,    // divide by 2     1:1     50.00% //JL
   16'b0000000011111111     // off
   };

wire  step0, step1, step2;

assign   step0 = PoR_bypass ? PoR_extern : PoR;

Persistence i_PS1 (
   .y(step1),
   .a(step0)
   );

wire  reset_ring_oscillator   = ~step0 & step1;
wire  enable_ring_oscillator  = ~step1;
assign  ring_oscillator_reset   = reset_ring_oscillator; //JLPL - 11_7_25

Persistence i_PS22 (
   .y(step2),
   .a(step1)
   );

assign
   boot_reset   = step2;

reg   start_ring_oscillator;
wire  alter_ring_oscillator; //JLPL - 11_7_25

`ifdef synthesis
wire tie_low   =1'b0;
wire tie_high  =1'b1;
DFFSRPQ_X1N_A7P5PP84TR_C14 i_SRO (.Q(start_ring_oscillator), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .R(alter_ring_oscillator), .SN(~reset_ring_oscillator), .D(1'b0), .CK(ref_clock));
`else
always @(posedge ref_clock or posedge reset_ring_oscillator)
   if       (reset_ring_oscillator) start_ring_oscillator <= #1 1'b1;
   else if  (alter_ring_oscillator) start_ring_oscillator <= #1 1'b0;
`endif
wire [2:0] Osc_ctrl;
wire Osc_en ;
assign Osc_ctrl = start_ring_oscillator ? 3'h0 : TCC[2:0];
assign Osc_en = start_ring_oscillator ? enable_ring_oscillator  : TCC[7];
`ifdef   synthesis
RingOsc_A9_C14 i_RO    (.clock(ref_clock), .d2(Osc_ctrl[2]), .d1(Osc_ctrl[1]), .d0(Osc_ctrl[0]), .EN(Osc_en));
`else

A2_RingOscillator i_RO (
   .RO    (ref_clock),
   .ctrl  ( start_ring_oscillator ? 3'h0 : TCC[2:0]),
   .enable( start_ring_oscillator ? enable_ring_oscillator  : TCC[7])
   );

`endif

// Divider
wire  [15:0]   ckctrl;
reg   [ 7:0]   ring;
wire  [ 3:0]   index = start_ring_oscillator ? 4'hf : TCC[6:3];

assign ckctrl = K1[16*index+:16];

always @(posedge ref_clock or posedge reset_ring_oscillator)
   if    (reset_ring_oscillator) ring[7:0] <= #1  8'hff;
   else begin
      if (ckctrl[08])   ring[0]   <= #1 ~&(ring[7:0] | ckctrl[7:0]);
      if (ckctrl[09])   ring[1]   <= #1    ring[0];
      if (ckctrl[10])   ring[2]   <= #1    ring[1];
      if (ckctrl[11])   ring[3]   <= #1    ring[2];
      if (ckctrl[12])   ring[4]   <= #1    ring[3];
      if (ckctrl[13])   ring[5]   <= #1    ring[4];
      if (ckctrl[14])   ring[6]   <= #1    ring[5];
      if (ckctrl[15])   ring[7]   <= #1    ring[6];
      end

wire  ring_clock = ring[0];

reg   [3:0] ZDIV;
always @(posedge ring_clock) if (reset_ring_oscillator) ZDIV <= `tCQ 4'h0;
                        else if ( TCC[10:8] != 3'b0   ) ZDIV <= `tCQ ZDIV + 1'b1;

assign   div_clock  = {{3{ZDIV[3]}},ZDIV[3:0],ring_clock} >> TCC[10:8];

reg   [1:0] BFSM; localparam [1:0] BOOT_INIT = 0, BOOT = 1, RELEASE = 2, RUNNING = 3;

always @(posedge div_clock)
   if (boot_reset) begin
      BFSM  <= `tCQ  BOOT_INIT;
      end
   else case (BFSM)
      BOOT_INIT :             BFSM <= `tCQ   BOOT;
      BOOT      : if (done)   BFSM <= `tCQ   RELEASE;
      RELEASE   :             BFSM <= `tCQ   RUNNING;
      default                 BFSM <= `tCQ   RUNNING;
      endcase

// Output
//assign   ref_reset =  boot_reset & ~(BFSM==RELEASE);
assign   ref_reset = ~(BFSM==RUNNING);
assign   boot      = ~boot_reset &  (BFSM==BOOT   );
assign   alter_ring_oscillator = (BFSM==RUNNING); //JLPL - 11_7_25

endmodule
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
`ifdef   verify_tilezero_boot

initial begin
   power_on_reset = 1'bx;
   #1000;
   power_on_reset = 1'b1;
   #35000;
   power_on_reset = 1'b0;
   #10;
   power_on_reset = 1'b1;
   #30;
   power_on_reset = 1'b0;
   #10000;
   TCC   = 3'b111;
   @(posedge RO); #1;
   alter_ring_oscillator = 1'b1;
   @(posedge RO); #1;
   alter_ring_oscillator = 1'b0;
   #10000;
   power_on_reset = 1'b1;
   #10000;
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
