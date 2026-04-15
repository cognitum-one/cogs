/*==============================================================================================================================

   Copyright © 2019 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : A2S
   Description          : I/O Addresses

================================================================================================================================
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================

   Notes for Use and Synthesis

==============================================================================================================================*/

localparam  [12:0]   // 13-bit IO Addresses -----------------------------------
   aST      =  13'h1fff,     // Status Register
   aSS      =  13'h1ffe,     // Stack Status Register
   aIN      =  13'h1ffd,     // Interrupt Register
   aIE      =  13'h1ffc,     // Interrupt Enable \ ~mask
   aEP      =  13'h1ffb,     // Interrupt Entry Point Register

   aFRM     =  13'h1ff1,     // Floating Point Rounding Mode
   aFCS     =  13'h1ff0,     // Floating Point Control and Status register

   aSCR     =  13'h1555,     // Scratch Register
   aCOM1    =  13'h03f8,     // COM 1

   aSHAdsgt =  13'h0000,     // SHA256 Co-processor digest output
   aSHAhash =  13'h0000,     // SHA256 Co-processor hash input
   aSHAmesg =  13'h0001,     // SHA256 Co-processor message input
   aSHAinit =  13'h0002,     // SHA256 Co-processor initialize input
   aSHAgo   =  13'h0003,     // SHA256 Co-processor start input

   aXS8data =  13'h0008,     // Xsalsa20\8 Co-processor data port
   aXS8aver =  13'h0009;     // Xsalsa20\8 Co-processor status port

// (r) denotes state guaranteed immediately after reset

localparam  [4:0]    // Status Register layout --------------------------------
   IEN      =  0,    // Interrupt Enable        1: enabled     (r)0: disabled
   SVR      =  1,    // User Mode            (r)1: supervisor     0: user
   PFC      =  2,    // Prefix Caught
   WFI      =  3;    // Wait for Interrupt      1: Stalls all clock except this flip-flop until interrupt

localparam  [4:0]    // Interrupt Names ---------------------------------------
   RU       =  0,    // R stack underflow
   SU       =  1,    // D stack underflow
   RF       =  2,    // R stack Full
   SF       =  3,    // D stack Full
   RE       =  4,    // R stack Empty
   SE       =  5,    // D stack Empty
   RT       =  6,    // R stack Threshold crossing
   DT       =  7,    // D stack Threshold crossing
   US       =  8,    // User
   EX       = 31;    // External Interrupt

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
