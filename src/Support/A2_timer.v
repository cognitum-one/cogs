//===============================================================================================================================
//
//    Copyright © 2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : A2S
//
//    Description          : Timer -- based on A2P4 timer!
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
//    --  To do a one-shot timeout.
//
//        VALUE  := 0x5555;         The value to be reload when COUNT reaches zero
//        SCALE  := 0xaa;           The scale factor to count between decrements of COUNT
//        COUNT  := 0x5555;         The initial value for COUNT -- for one-shot should be the same as VALUE
//        CSR    := 0x2;
//
//        On Interrupt:             The hardware will automatically clear CSR bit 1 to a 0.
//
//        To clear the interrupt:   CSR := 0;
//
//    --  To do a continuous timer
//
//        VALUE  := 0x5555;         The value to reload when COUNT reaches zero
//        SCALE  := 0xaa;           The scale factor to count between decrements of COUNT
//        COUNT  := 0x5555;         The initial value for COUNT need not be the same as VALUE!!
//        CSR    := 0x1;
//
//        On Interrupt:             The hardware will automatically reload COUNT with VALUE and continue
//
//    --  To clear the interrupt:   TMR_CSR := 1;    -- to keep the timer running bit 0 must still be set but we must clear
//                                                      bit 2 to reset the interrupt!
//
//===============================================================================================================================

module   A2_timer #(
   parameter   [12:0]   BASE     = 13'h00000,
   parameter            NUM_REGS = 5,
   parameter            WIDTH    = 32
   )(
   // A2S I/O interface
   output wire [WIDTH:0]      imiso,   // Slave data out {               IOk, IOo[31:0]}
   input  wire [WIDTH+15:0]   imosi,   // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
   // Global
   output wire                intpt,   // Evaluation complete interrupt -- cleared by taking ENA low
   input  wire                reset,
   input  wire                clock
   );

localparam  LG2REGS     =   `LG2(NUM_REGS);
localparam  INTERRUPT   = 2, ONE_SHOT = 1, CONTINUOUS = 0;

// Interface decoding ===========================================================================================================

wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
wire            IOk;
// Decollate imosi
assign   {cIO, iIO, aIO} = imosi;

wire                  in_range =  aIO[12:LG2REGS] == BASE[12:LG2REGS];
wire  [NUM_REGS-1:0]    decode =  in_range << aIO[LG2REGS-1:0];
wire  [NUM_REGS-1:0]      read =  {NUM_REGS{(cIO==3'b101)}} & decode;
wire  [NUM_REGS-1:0]     write =  {NUM_REGS{(cIO==3'b110)}} & decode;

// Locals =======================================================================================================================

reg   [WIDTH-1:0] CSR, VALUE, SCALE, LOWER, UPPER;

wire  [2:1] n_csr;

wire        active            =  CSR[CONTINUOUS] | CSR[ONE_SHOT],
            lower_zero        =  LOWER == 32'b 0,
            upper_zero        =  UPPER == 32'b 0,
            finish            =  lower_zero & upper_zero,
            reload_lower      =  active & lower_zero,
            reload_upper      =  CSR[CONTINUOUS] & finish,
            decrement_lower   =  active,
            decrement_upper   =  active & lower_zero;
assign
            n_csr[INTERRUPT]  =  CSR[INTERRUPT]  | (finish & active),
            n_csr[ONE_SHOT ]  =  CSR[ONE_SHOT]   & ~finish;

// Registers ====================================================================================================================

always @(posedge clock)
   if      (reset          )  CSR       <= `tCQ  32'h0;
   else if (write[CSR]     )  CSR       <= `tCQ  iIO;
   else if (active         )  CSR[2:1]  <= `tCQ  n_csr[2:1];

always @(posedge clock)
   if      (reset          )  VALUE     <= `tCQ  32'h0;
   else if (write[VALUE]   )  VALUE     <= `tCQ  iIO;

always @(posedge clock)
   if      (reset          )  SCALE     <= `tCQ  32'h0;
   else if (write[SCALE]   )  SCALE     <= `tCQ  iIO;

always @(posedge clock)
   if      (reset          )  LOWER     <= `tCQ  64'h0;
   else if (write[LOWER]   )  LOWER     <= `tCQ  iIO;
   else if (reload_lower   )  LOWER     <= `tCQ  SCALE;
   else if (decrement_lower)  LOWER     <= `tCQ  LOWER - 1'b1;

always @(posedge clock)
   if      (reset          )  UPPER     <= `tCQ  64'h0;
   else if (write[UPPER]   )  UPPER     <= `tCQ  iIO;
   else if (reload_upper   )  UPPER     <= `tCQ  VALUE;
   else if (decrement_upper)  UPPER     <= `tCQ  UPPER - 1'b1;

// Output interface =============================================================================================================

A2_mux #(NUM_REGS,WIDTH) i_IOR (.z(IOo), .s(read), .d({UPPER, LOWER, SCALE, VALUE, CSR}));

assign   IOk   = |decode & cIO[2],
         imiso = {IOk, IOo},
         intpt =  CSR[INTERRUPT];

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
