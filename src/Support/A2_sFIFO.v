/********************************************************************************************************************************
**
**    Copyright © 1998..2006 Advanced Architectures
**
**    All rights reserved
**    Confidential Information
**    Limited Distribution to Authorized Persons Only
**    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
**
**    Project Name         : A2B Synchronous System Bus
**    Description          : Synchronous FIFO with Flag outputs
**
**    Author               : RTT
**    Creation Date        : 1/5/1998
**    Version Number       : 0.1
**
********************************************************************************************************************************
**    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
**    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
**    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
**    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
**    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
**    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
********************************************************************************************************************************
**    Notes for Use and Synthesis
**
**    1. Logic is very much simplified when ae_threshhold and af_threshhold are powers of 2
**    2. Beware of large fan-outs of PUSH and POP for increasing widths of data
**
********************************************************************************************************************************/

module   A2_sFIFO #(
   parameter   WIDTH          =  32,
   parameter   DEPTH          =   1,
   parameter   FALLTHROUGH    =   1,   // Set to 0 for asynchronous fallthrough mode i.e. No clocks from in to out if empty
   parameter   AF_THRESHHOLD  =   1,
   parameter   AE_THRESHHOLD  =   1
   ) (
   output wire [WIDTH-1 :0]   data_out,
   output wire                empty,
   output wire                ae,
   output wire                af,
   output wire                full,

   input  wire [WIDTH-1 :0]   data_in,
   input  wire                push,
   input  wire                pop,

   input  wire                clock,
   input  wire                reset
   );

localparam  PTR_WIDTH   =     `LG2(DEPTH);

reg      [PTR_WIDTH-1   :0]   rd_pointer;
reg      [PTR_WIDTH-1   :0]   wr_pointer;
reg      [PTR_WIDTH     :0]   dipstick;

reg      [WIDTH-1       :0]   two_port_ram   [0:DEPTH-1];
reg      [WIDTH-1       :0]   ram_out;
reg      [WIDTH-1       :0]   bypass_data;
reg                           sync_bypass_flag;

wire     [PTR_WIDTH-1   :0]   nxt_rd_pointer;
wire     [PTR_WIDTH-1   :0]   nxt_wr_pointer;
wire                          ptrs_equal;

// Initialize ------------------------------------------------------------------------------------------------------------------

`ifndef synthesis
// synopsys  translate_off
// synthesis translate_off

initial begin : initialize_ram
integer     i;
   for (i=0; i<DEPTH; i=i+1) two_port_ram[i] <= {WIDTH{1'bx}};
end

// synthesis translate_on
// synopsys  translate_on
`endif

// Pointer & Counters ----------------------------------------------------------------------------------------------------------

assign   ptrs_equal     =  wr_pointer == rd_pointer;

assign   nxt_wr_pointer =  wr_pointer == (DEPTH[PTR_WIDTH-1:0] - 1'b 1) ? {PTR_WIDTH{1'b 0}}
                        :  wr_pointer + 1'b 1;

assign   nxt_rd_pointer =  rd_pointer == (DEPTH[PTR_WIDTH-1:0] - 1'b 1) ? {PTR_WIDTH{1'b 0}}
                        :  rd_pointer + 1'b 1;

always @(posedge clock)
   if (reset) begin
      rd_pointer  <= `tCQ  0;
      wr_pointer  <= `tCQ  0;
      dipstick    <= `tCQ  0;
      end
   else begin
      wr_pointer  <= `tCQ  push && !pop &&           !full
                     ||    push &&  pop
                     ||    push &&         !empty && !full
                     ||    push &&                   !full &&  FALLTHROUGH   ?  nxt_wr_pointer
                                                                             :      wr_pointer;

      rd_pointer  <= `tCQ !push &&  pop && !empty && !ptrs_equal
                     ||   !push &&  pop &&  empty && !ptrs_equal
                     ||    push && !pop &&  empty
                     ||    push &&  pop && !empty
                     ||    push &&  pop &&  empty &&           FALLTHROUGH   ?  nxt_rd_pointer
                     :                                                              rd_pointer;

      dipstick    <= `tCQ !push &&  pop && !empty && !full
                     ||   !push &&  pop && !empty &&  full                   ?  dipstick - 1'b 1
                     :     push && !pop && !empty && !full
                     ||    push && !pop &&  empty && !full
                     ||    push && !pop &&  empty &&  full
                     ||    push &&  pop &&  empty && !full &&  FALLTHROUGH   ?  dipstick + 1'b 1
                      :                                                         dipstick;
      end

// Two Port Synchronous SRAM ---------------------------------------------------------------------------------------------------

always @(posedge clock) begin
   if (push)
      two_port_ram[wr_pointer]   <= `tCQ  data_in;
   if (pop)
      ram_out  <= `tCQ  two_port_ram[rd_pointer];
   end

// Synchronous Bypass ---------------------------------------------------------------------------------------------------------
// Note that this memory supports bypass if read and write addresses are identical
// The extra read when push and empty allows the FIFO to operate as though it has an asynchonous RAM output
// A standard synchronous RAM behaves like a two levels of registers when transitioning from empty i.e. a fallthrough of two
// This bypass causes the behavior to be like a single register when transitioning from empty      i.e. a fallthrough of one
// For a fallthrough of zero see the Asynchronous bypass mode below

always @(posedge clock)
   if (push && empty || push && pop && ptrs_equal)
      bypass_data <= `tCQ data_in;

always @(posedge clock) begin
   if       (reset                     )  sync_bypass_flag  <= `tCQ 1'b 0;
   else if  (push && empty             )  sync_bypass_flag  <= `tCQ 1'b 1;
   else if  (push && pop && ptrs_equal )  sync_bypass_flag  <= `tCQ 1'b 1;
   else if  (pop                       )  sync_bypass_flag  <= `tCQ 1'b 0;
   end

// Asynchronous Bypass ---------------------------------------------------------------------------------------------------------

assign   data_out =  (FALLTHROUGH == 0) && empty   ?  data_in
                  :   sync_bypass_flag             ?  bypass_data
                  :                                   ram_out;

// Flag Outputs ----------------------------------------------------------------------------------------------------------------
assign
   empty    =  dipstick ==  {PTR_WIDTH{1'b 0}},
   ae       =  dipstick <=                     AE_THRESHHOLD[PTR_WIDTH:0],
   af       =  dipstick >= (DEPTH[PTR_WIDTH:0]-AF_THRESHHOLD[PTR_WIDTH:0]- 1'b 1),
   full     =  dipstick ==  DEPTH[PTR_WIDTH:0];

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef verify_A2_sFIFO

module   t;

wire [31:0] data_out;
wire        empty;
wire        ae;
wire        af;
wire        full;

reg  [31:0] data_in;
reg         push;
reg         pop;
reg         clock;
reg         reset;

A2_sFIFO #(
   .WIDTH          (32),
   .DEPTH          (32),
   .FALLTHROUGH    ( 1),
   .AF_THRESHHOLD  ( 1),
   .AE_THRESHHOLD  ( 1)
   ) mut (
   .data_out   (data_out),
   .pop        (pop),
   .ae         (ae),
   .empty      (empty),

   .data_in    (data_in),
   .push       (push),
   .af         (af),
   .full       (full),

   .clock      (clock),
   .reset      (reset)
   );

endmodule

`endif
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

